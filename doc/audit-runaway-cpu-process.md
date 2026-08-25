# Audit: orphaned `dan` process at 100% CPU (macOS)

Date: 2026-08-25. Reproduced and root-caused with a scripted pty harness
against `target/release/dan` built from `db2ade2` (0.2.181).

## Root cause (confirmed by stack sample)

When the terminal goes away while `dan` is running — window/tab closed,
SSH drop, tmux pane killed, terminal emulator crash — the pty is hung up and
`read(2)` on the tty starts returning EOF / `EIO`. crossterm 0.29's mio event
backend (`crossterm-0.29.0/src/event/source/unix/mio.rs`, `try_read`, the
`TTY_TOKEN` inner `loop`) handles only two read outcomes:

- `Err(WouldBlock)` → `break`
- `Err(Interrupted)` → `continue`

`Ok(0)` (EOF) and any other error (`EIO`) fall through and the loop re-issues
`read()` immediately. `event::poll()` never returns, so `run_loop` never sees
`should_quit` or the signal flag, and the main thread spins at 100% CPU.

Sampled stack of the spinning process (`sample <pid>`):

```
dan::main
  crossterm::event::poll
    crossterm::event::read::InternalEventReader::poll
      UnixInternalEventSource::try_read        (mio.rs)
        rustix::backend::io::syscalls::read
          read  (libsystem_kernel)             ← ~99% of samples
```

The process has no terminal any more, so nothing is ever printed — which is
why it "leaves no terminal trace". It is not a normal-quit leak: Ctrl-Q
through a real shell exits cleanly (`DAN_EXIT=0`, verified). The observation
"looks like it quit normally" is most likely a *different* dan instance (another
tab / window / ssh session) whose terminal was closed while it was still open.

## Why the installed binary still shows it

- `main.rs` gained `spawn_shutdown_watchdog` (commit `552a7f7`, 2026-07-09):
  on SIGHUP/SIGTERM/SIGINT/SIGQUIT it force-exits after 2 s if the main loop
  hasn't quit. That converts the spin into ~1–2 s of 100% CPU and then exit.
  Verified: with SIGHUP delivered the orphan is at 95.9% CPU at t+1 s and gone
  at t+2 s.
- The installed `~/.cargo/bin/dan` reports `0.2.169 (124e8b6)`; commit
  `124e8b6` (2026-07-10, no longer on any branch) contains **no**
  `spawn_shutdown_watchdog` — it predates the watchdog. Any hang-up with that
  binary spins forever.

Even with the watchdog, the spin persists whenever SIGHUP does **not** reach
dan. Reproduced by starting dan with SIGHUP blocked and closing the pty:
100% CPU indefinitely (killed after 6 s). Real-world cases where SIGHUP is not
delivered or arrives late: dan not in the tty's foreground process group
(backgrounded / suspended job), terminals or multiplexers that only close the
master fd, hosts where the shell traps HUP. The watchdog is a backstop, not a
fix.

## Ruled out (tested or read)

| Candidate | Result |
|---|---|
| Normal Ctrl-Q quit path | clean exit, rc 0 |
| 0×0 resize (`SIGWINCH` with zero winsize) | no spin; quits normally afterwards |
| 1×1 terminal | no spin |
| Formatter thread / child (`formatter.rs`) | `shutdown_async_work` kills child; thread blocked in `wait_with_output`, not spinning; killed at `exit()` |
| Project index walker (`palette/index.rs`) | exits on first failed `send` after `project_index_rx = None`; not a spin |
| Autosave / recent-files writer threads | short-lived; killed at `exit()` |
| Watchdog thread itself | `sleep(100ms)` loop, 0% CPU |
| nucleo | only the synchronous `Matcher`, no thread pool |
| arboard / terminal-colorsaurus / signal-hook | no long-lived threads; colorsaurus only runs at startup |
| Rust `exit()` hanging in state `E` | artifact of the harness making dan the session leader; not reproducible under a real shell |
| crossterm `use-dev-tty` backend (`tty.rs`) | handles `Ok(0)` by breaking, but `poll(2)` then reports `POLLHUP` immediately → still a busy loop (bounded by the poll timeout, so `run_loop` regains control) — not a fix on its own |

Side observation (not CPU-related): if the terminal stops draining (XOFF /
Ctrl-S, frozen emulator) the main thread blocks in `write(2)` inside
`ScreenBuffer::diff` → `BufWriter::flush`. Sampled during the harness runs when
the pty master wasn't drained. 0% CPU; already noted in the watchdog comments.

## Fix applied (2026-08-25)

`src/main.rs`: the shutdown watchdog thread now also probes fd 0 every 100 ms
with a zero-timeout `poll(2)` (`tty_hung_up()`); when the pty reports
`POLLHUP`/`POLLERR`/`POLLNVAL` it flips the same shutdown flag the signal
handlers use, so the existing 2 s grace + force-exit path runs even when no
SIGHUP is delivered. Guarded by `isatty(0)` so a piped stdin closing is not
mistaken for a hang-up. `libc = "0.2"` added under `[target.'cfg(unix)']`.

Note for macOS: `poll` must request `POLLIN` — with `events = 0` a hung-up
slave reports nothing at all (verified); with `POLLIN` it reports
`POLLIN | POLLHUP`. Only the `POLLHUP` bit is acted on, so pending keyboard
input never triggers it.

Verified with the harness: SIGHUP-blocked hang-up now exits within ~1 s
(previously spun indefinitely); SIGHUP-delivered hang-up unchanged; normal
Ctrl-Q quit unchanged; `cargo test --release` green.

A fix inside `run_loop` cannot work on its own: the hang-up arrives while the
main thread is already inside `event::poll`, which never returns.

Still worth doing upstream: crossterm's `mio.rs` should return `Err` on
`Ok(0)`/`EIO` instead of looping. And **reinstall** (`cargo install --path .`)
— the installed 0.2.169 has neither the watchdog nor this probe.

Harness caveat: the scripted Ctrl-Q occasionally is not acted on (screen
re-renders instead of quitting) on the unmodified tree as well — a timing
artifact of the harness sending the key ~2 s after spawn, not a regression.
Not investigated further.

## Reproduction

`scratchpad/ptytest4.py` (session scratch, not in repo) — spawn `zsh -c dan …`
on a `pty.openpty()` pair with `setsid` + `TIOCSCTTY`, optionally block
SIGHUP in the child, close the master fd, watch `ps -o %cpu,stat` and
`sample <pid>`.
