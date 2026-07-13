// SPDX-License-Identifier: GPL-3.0-or-later

/*
 * Dan -- a fast, friendly, and zero-fuss terminal text editor.
 * Copyright (C) 2026 Daniel Fallman
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
*/
	
mod buffer;
mod config;
mod atomic_io;
mod crash;
mod editor;
mod input;
mod palette;
pub mod recovery;
mod render;
mod sanitize;
mod syntax;
pub mod ui;
mod utils;
mod terminal_guard;

use crossterm::event::{self, Event};

use std::env;
use std::io::{self, BufWriter};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::editor::Editor;

/// Version from the VERSION file (embedded at compile time).
pub const VERSION: &str = include_str!("../VERSION");

/// Short git hash (embedded at compile time by build.rs).
pub const GIT_HASH: &str = env!("GIT_HASH");

/// Register handlers for SIGTERM, SIGHUP, SIGQUIT, and SIGINT that flip the
/// returned flag. The main loop polls this flag and triggers a graceful
/// shutdown, ensuring the terminal is restored from raw mode + alt screen
/// instead of being stranded after `kill` / SSH drop / parent-process exit.
/// On non-Unix the returned flag is never flipped.
fn install_signal_shutdown_flag() -> io::Result<Arc<AtomicBool>> {
	let flag = Arc::new(AtomicBool::new(false));
	#[cfg(unix)]
	{
		use signal_hook::consts::{SIGHUP, SIGINT, SIGQUIT, SIGTERM};
		signal_hook::flag::register(SIGTERM, Arc::clone(&flag))?;
		signal_hook::flag::register(SIGHUP, Arc::clone(&flag))?;
		signal_hook::flag::register(SIGQUIT, Arc::clone(&flag))?;
		signal_hook::flag::register(SIGINT, Arc::clone(&flag))?;
	}
	Ok(flag)
}

/// How long the main loop gets to honour a shutdown request before the watchdog
/// terminates the process itself.
const SHUTDOWN_GRACE: Duration = Duration::from_secs(2);

/// How long the watchdog waits for the buffer rescue + terminal restore before
/// exiting anyway. Bounded so a restore that itself blocks cannot strand us.
const RESCUE_BUDGET: Duration = Duration::from_secs(2);

/// Force-exit if the main loop does not honour a shutdown request.
///
/// The signal handlers only flip an `AtomicBool` that `run_loop` polls, so a
/// main loop that is wedged never sees it: the process ignores SIGTERM/SIGHUP,
/// spins at 100% CPU, and can only be killed with SIGKILL — typically after the
/// terminal it was attached to is already gone. That turns any editor hang into
/// an orphaned CPU-burning process, so a graceful-shutdown flag alone is not
/// enough. This thread is the backstop: if the flag is still set once the grace
/// period elapses, the loop is not coming back, and we rescue unsaved buffers,
/// restore the terminal, and exit without it.
#[cfg(unix)]
fn spawn_shutdown_watchdog(flag: Arc<AtomicBool>) {
	std::thread::spawn(move || loop {
		if !flag.load(Ordering::Relaxed) {
			std::thread::sleep(Duration::from_millis(100));
			continue;
		}

		// Shutdown requested. A healthy main loop exits well inside the grace
		// period and takes this thread down with the process; if we are still
		// running when it elapses, it is wedged.
		std::thread::sleep(SHUTDOWN_GRACE);

		// Rescue and restore on a helper thread: if either blocks (a stuck tty
		// write, a full disk), the timeout below still gets us to `_exit`.
		let (tx, rx) = std::sync::mpsc::channel();
		std::thread::spawn(move || {
			let rescued = crate::crash::dump();
			emergency_terminal_restore();
			let _ = tx.send(rescued);
		});
		let rescued = rx.recv_timeout(RESCUE_BUDGET).unwrap_or_default();

		let mut msg =
			String::from("\r\ndan: main loop unresponsive to shutdown signal; forcing exit.\r\n");
		for p in &rescued {
			msg.push_str(&format!("dan: rescued unsaved buffer to {}\r\n", p.display()));
		}
		write_fd(2, msg.as_bytes());

		// _exit(2): no atexit handlers, no stdio flush — the wedged thread may
		// hold the stdout lock, and blocking on it here would defeat the point.
		signal_hook::low_level::exit(1);
	});
}

#[cfg(not(unix))]
fn spawn_shutdown_watchdog(_flag: Arc<AtomicBool>) {}

/// Undo raw mode, the alternate screen, mouse capture and bracketed paste from
/// a thread that cannot trust the main thread to still be alive.
#[cfg(unix)]
fn emergency_terminal_restore() {
	const RESTORE: &str = concat!(
		"\x1b[?2004l", // bracketed paste off
		"\x1b[?1000l", "\x1b[?1002l", "\x1b[?1003l", "\x1b[?1006l", // mouse reporting off
		"\x1b[0m",     // reset colours + attributes
		"\x1b[?25h",   // show cursor
		"\x1b[?1049l", // leave alternate screen
	);
	write_fd(1, RESTORE.as_bytes());
	let _ = crossterm::terminal::disable_raw_mode();
}

/// `write(2)` straight to a file descriptor, bypassing Rust's buffered and
/// mutex-guarded stdio handles — the wedged main thread may be holding them.
#[cfg(unix)]
fn write_fd(fd: std::os::fd::RawFd, mut bytes: &[u8]) {
	use std::io::Write;
	use std::mem::ManuallyDrop;
	use std::os::fd::FromRawFd;

	// ManuallyDrop: this File only borrows the fd; dropping it would close
	// stdout/stderr out from under the process.
	let mut f = ManuallyDrop::new(unsafe { std::fs::File::from_raw_fd(fd) });
	while !bytes.is_empty() {
		match f.write(bytes) {
			Ok(0) => break,
			Ok(n) => bytes = &bytes[n..],
			Err(ref e) if e.kind() == io::ErrorKind::Interrupted => {}
			Err(_) => break,
		}
	}
}

fn main() -> io::Result<()> {
	let args: Vec<String> = env::args().collect();

	// Handle version flags: -v, --v, --version
	if args.len() > 1 {
		let flag = args[1].as_str();
		if matches!(flag, "-v" | "--v" | "--version") {
			println!("dan {} ({})", VERSION.trim(), GIT_HASH);
			return Ok(());
		}
	}

	let mut editor = Editor::new();

	// Open file(s) from arguments
	if args.len() > 1 {
		let path = Path::new(&args[1]);
		if path.exists() {
			if let Err(e) = editor.open_file(path) {
				eprintln!("dan: Could not open '{}': {}", path.display(), e);
				std::process::exit(1);
			}
		} else {
			// Create a new buffer with the target path for saving
			editor.buffer_mut().file_path = Some(path.to_path_buf());
			// Swap path keyed to the target so autosave / crash recovery cover
			// edits to a not-yet-existing file (P0-1).
			editor.buffer_mut().swp_path = Some(crate::recovery::get_swap_path(path));
			editor.config.apply_editorconfig(path); // Apply layout rules perfectly even for new files!
			editor.set_status(format!("[New File] {}", args[1]));
		}
	} else {
		// No file argument: greet the user with the palette open. The startup
		// scratch [Untitled] is still there as a placeholder, but it's hidden
		// behind the palette and gets auto-disposed as soon as the user picks
		// a file or "New buffer" — so they never see a stray buffer they
		// didn't ask for.
		editor.set_status("dan's text editor | ^Q to quit");
		editor.execute(crate::editor::commands::Command::PaletteOpen);
	}

	// Install signal handlers BEFORE entering raw mode so a signal arriving
	// during the next few statements still trips the cleanup path.
	let shutdown_signal = install_signal_shutdown_flag()?;

	// Backstop the flag: a wedged main loop never polls it, and would otherwise
	// survive SIGTERM/SIGHUP and spin at 100% CPU until SIGKILLed.
	spawn_shutdown_watchdog(Arc::clone(&shutdown_signal));

	// Install the panic hook BEFORE enable_raw_mode so a panic between
	// raw-mode-on and the first hook installation can no longer strand
	// the terminal. The hook's disable_raw_mode / LeaveAlternateScreen
	// calls are wrapped in `let _ =` and are no-ops if those modes were
	// never entered, so installing early is safe.
	let default_panic_hook = std::panic::take_hook();
	std::panic::set_hook(Box::new(move |panic_info| {
		// Rescue unsaved buffers to their swap files BEFORE anything else, so a
		// secondary failure during terminal restore can't cost the user's work.
		let rescued = crate::crash::dump();

		let mut stdout = io::stdout();
		let _ = crossterm::ExecutableCommand::execute(
			&mut stdout,
			crossterm::event::DisableBracketedPaste,
		);
		let _ = crossterm::ExecutableCommand::execute(
			&mut stdout,
			crossterm::event::DisableMouseCapture,
		);
		let _ = crossterm::ExecutableCommand::execute(&mut stdout, crossterm::style::ResetColor);
		let _ = crossterm::ExecutableCommand::execute(
			&mut stdout,
			crossterm::style::SetAttribute(crossterm::style::Attribute::Reset),
		);
		let _ = crossterm::ExecutableCommand::execute(&mut stdout, crossterm::cursor::Show);
		let _ = crossterm::ExecutableCommand::execute(
			&mut stdout,
			crossterm::cursor::SetCursorStyle::DefaultUserShape,
		);
		let _ = crossterm::ExecutableCommand::execute(
			&mut stdout,
			crossterm::terminal::LeaveAlternateScreen,
		);
		let _ = crossterm::terminal::disable_raw_mode();
		default_panic_hook(panic_info);

		if !rescued.is_empty() {
			eprintln!("\ndan: rescued {} unsaved buffer(s) before exiting:", rescued.len());
			for p in &rescued {
				eprintln!("  {}", p.display());
			}
			eprintln!("Reopen the affected file(s) to recover, or inspect the .swp directly.");
		}
	}));

	// Set up terminal (restored automatically by TerminalGuard on drop).
	let mut terminal = terminal_guard::TerminalGuard::enter(editor.config.mouse)?;
	let writer = terminal.writer_mut();

	// Flush stray terminal-query replies before the first frame.
	//
	// Editor::new() asks the terminal for its fg/bg colours (OSC 10/11, via
	// terminal-colorsaurus) to auto-pick a light/dark theme. colorsaurus tells
	// query-capable terminals apart from the rest by also sending a DA1 request
	// and assuming replies come back in order — a DA1 answer before the colour
	// answers is read as "unsupported". Some terminals (e.g. Terax) answer DA1
	// *first* despite fully supporting the colour query, so colorsaurus bails
	// and the colour replies arrive late, landing in our stdin. crossterm then
	// parses them as key input, typing the raw reply ("10;rgb:…11;rgb:…") into
	// the buffer on the first frame. When the query failed, drain that leftover
	// input. The replies come as one burst, so stopping after a brief quiet gap
	// catches them without swallowing real keystrokes (nobody types this fast
	// the instant the editor launches).
	if editor.color_query_failed {
		while event::poll(Duration::from_millis(20))? {
			let _ = event::read()?;
		}
	}

	// Main loop
	let result = run_loop(&mut editor, writer, &shutdown_signal);

	editor.shutdown_async_work();

	// Flush recent-files to disk on shutdown — synchronous so the write
	// definitely completes before the process exits.
	if editor.recent_files_dirty {
		let snap: Vec<_> = editor.recent_files.iter().cloned().collect();
		crate::palette::index::save_recent_files(&snap);
	}

	// Clean up untitled crash-dump swaps on a clean exit: they can never be
	// auto-offered for recovery (no originating file to key on), so leaving them
	// behind would only litter $TMPDIR. File-backed swaps are intentionally left
	// for the save/discard paths to manage.
	for buf in &editor.buffers {
		if buf.file_path.is_none() {
			if let Some(ref swp) = buf.swp_path {
				crate::recovery::cleanup_swap(swp);
			}
		}
	}

	result
}

fn run_loop(
	editor: &mut Editor,
	writer: &mut BufWriter<io::Stdout>,
	shutdown_signal: &Arc<AtomicBool>,
) -> io::Result<()> {
	loop {
		// A SIGTERM/SIGHUP/SIGQUIT triggers graceful shutdown via the
		// existing terminal-restoration path in `main`. The most-recent
		// state should already be in the autosave swap-file (5s cadence).
		if shutdown_signal.load(Ordering::Relaxed) {
			editor.should_quit = true;
		}

		render::render(editor, writer)?;

		if editor.should_quit {
			break;
		}

		// Wait for an event, polling async tasks continuously.
		// Tight 25 ms poll while a formatter task is in flight (we want its
		// result rendered as soon as it lands); 200 ms while the project
		// indexer is walking; otherwise relax to 500 ms so an idle editor isn't
		// waking the CPU 40×/sec. Autosave (5 s cadence) is unaffected;
		// keystrokes are delivered immediately because `event::poll` returns as
		// soon as stdin is readable.
		let evt = loop {
			if shutdown_signal.load(Ordering::Relaxed) {
				editor.should_quit = true;
				return Ok(());
			}
			let did_work = editor.poll_async_tasks();
			if did_work {
				render::render(editor, writer)?;
			}

			let any_formatting = editor.buffers.iter().any(|b| b.fmt_rx.is_some());
			let indexing = editor.project_index_rx.is_some();
			let poll_timeout = if any_formatting {
				Duration::from_millis(25)
			} else if indexing {
				Duration::from_millis(200)
			} else {
				Duration::from_millis(500)
			};
			if event::poll(poll_timeout)? {
				break event::read()?;
			}
		};

		if matches!(evt, Event::Key(_) | Event::Paste(_))
			&& editor.mode != crate::editor::mode::Mode::Searching
			&& editor.mode != crate::editor::mode::Mode::ConfirmQuit
			&& editor.mode != crate::editor::mode::Mode::SaveAs
			&& editor.mode != crate::editor::mode::Mode::Palette
			&& editor.mode != crate::editor::mode::Mode::RecoverSwap
		{
			editor.clear_status();
			editor.clear_info_banner();
		}

		// Handle resize events directly (not routed through Command).
		if let Event::Resize(w, h) = evt {
			editor.handle_resize(w, h);
		}

		let cmd = input::map_event(&evt, editor.mode);
		editor.execute(cmd);

		// Drain extra buffered events with a 5ms micro-timeout so a burst
		// of key events (fast typing, continuous scrolling) collapses into
		// one render pass.
		while event::poll(Duration::from_millis(5))? {
			let evt = event::read()?;
			if matches!(evt, Event::Key(_) | Event::Paste(_))
				&& editor.mode != crate::editor::mode::Mode::Searching
				&& editor.mode != crate::editor::mode::Mode::ConfirmQuit
				&& editor.mode != crate::editor::mode::Mode::SaveAs
				&& editor.mode != crate::editor::mode::Mode::Palette
				&& editor.mode != crate::editor::mode::Mode::RecoverSwap
			{
				editor.clear_status();
				editor.clear_info_banner();
			}
			if let Event::Resize(w, h) = evt {
				editor.handle_resize(w, h);
			}
			let cmd = input::map_event(&evt, editor.mode);
			editor.execute(cmd);
		}

		// Refresh the crash registry so the panic hook can rescue the latest
		// dirty-buffer state if the next iteration panics (P0-2).
		editor.publish_crash_snapshot();
	}

	Ok(())
}
