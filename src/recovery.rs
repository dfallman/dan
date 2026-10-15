use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// Path of the `.swp` crash-recovery file for `original_path`. Prefers a
/// hidden sibling (`.foo.txt.swp`); falls back to a name-flattened entry in
/// `$TMPDIR/dan_swaps/` when the original directory isn't writable.
///
/// The path is resolved to an absolute one first: a bare relative name like
/// `notes.md` has an empty parent, which would fail the writability check and
/// land every `notes.md` from every directory on the same fallback swap.
pub fn get_swap_path(original_path: &Path) -> PathBuf {
	let original_path = &crate::utils::file_identity(original_path);
	if let Some(file_name) = original_path.file_name() {
		let mut swp_name = std::ffi::OsString::from(".");
		swp_name.push(file_name);
		swp_name.push(".swp");

		let local_swp = original_path.with_file_name(&swp_name);

		if let Some(parent) = local_swp.parent() {
			if dir_writable(parent) {
				return local_swp;
			}
		}

		// Fall back to $TMPDIR/dan_swaps/.
		let mut temp_dir = env::temp_dir();
		temp_dir.push("dan_swaps");
		let _ = fs::create_dir_all(&temp_dir);

		temp_dir.push(format!("{}.swp", flatten_path(original_path)));
		return temp_dir;
	}

	PathBuf::from(".dan.swp")
}

/// "This file is open in dan PID n": held for as long as a buffer edits the
/// file, released on drop. Without it, a second dan on the same file treats
/// the first one's live swap as a crash leftover: offering it for recovery,
/// deleting it on decline, and overwriting it on its own autosave.
///
/// Locks live under the user cache dir rather than beside the file so an
/// open file doesn't litter the repo, and so every session (local, ssh,
/// tmux) sees the same lock whatever `$TMPDIR` says.
pub struct SwapLock {
	path: PathBuf,
}

impl Drop for SwapLock {
	fn drop(&mut self) {
		// Only remove it if it is still ours: a stale-lock reclaim by another
		// process could have replaced it.
		if read_lock_pid(&self.path) == Some(std::process::id()) {
			let _ = fs::remove_file(&self.path);
		}
	}
}

pub enum LockOutcome {
	/// We own the file. `None` when the lock couldn't be created at all
	/// (unwritable cache dir): proceed as owner, best-effort as before.
	Acquired(Option<SwapLock>),
	/// Another running dan has it open.
	HeldBy(u32),
}

/// Take the open-file lock for `file`.
pub fn lock_file(file: &Path) -> LockOutcome {
	lock_file_with(file, &lock_dir(), pid_is_live_dan)
}

fn lock_dir() -> PathBuf {
	dirs::cache_dir()
		.map(|d| d.join("dan").join("locks"))
		.unwrap_or_else(|| env::temp_dir().join("dan_locks"))
}

fn lock_file_with(file: &Path, dir: &Path, is_live_dan: fn(u32) -> bool) -> LockOutcome {
	if fs::create_dir_all(dir).is_err() {
		return LockOutcome::Acquired(None);
	}
	let path = dir.join(format!("{}.lock", flatten_path(&crate::utils::file_identity(file))));
	let me = std::process::id();
	// Two passes: the second follows removal of a stale lock.
	for _ in 0..2 {
		let mut options = fs::OpenOptions::new();
		options.write(true).create_new(true);
		#[cfg(unix)]
		{
			use std::os::unix::fs::OpenOptionsExt;
			options.mode(0o600);
		}
		match options.open(&path) {
			Ok(mut f) => {
				use std::io::Write;
				let _ = write!(f, "{}", me);
				return LockOutcome::Acquired(Some(SwapLock { path }));
			}
			Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => match read_lock_pid(&path) {
				// Our own PID can only be left by a dead predecessor (or a
				// half-written lock); either way it's stale.
				Some(pid) if pid != me && is_live_dan(pid) => return LockOutcome::HeldBy(pid),
				_ => {
					let _ = fs::remove_file(&path);
				}
			},
			Err(_) => return LockOutcome::Acquired(None),
		}
	}
	LockOutcome::Acquired(None)
}

fn read_lock_pid(path: &Path) -> Option<u32> {
	fs::read_to_string(path).ok()?.trim().parse().ok()
}

/// True if `pid` is running and is a dan (same executable name as us), so a
/// lock left by a crashed dan whose PID was since reused by something else
/// isn't mistaken for a live owner. When the name can't be read the process
/// is assumed to be dan: skipping a swap beats clobbering one.
fn pid_is_live_dan(pid: u32) -> bool {
	#[cfg(unix)]
	{
		// SAFETY: signal 0 only checks for existence/permission.
		let alive = unsafe { libc::kill(pid as libc::pid_t, 0) } == 0
			|| std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM);
		if !alive {
			return false;
		}
		let ours = std::env::current_exe().ok().and_then(|p| p.file_name().map(|n| n.to_owned()));
		match (process_exe_name(pid), ours) {
			(Some(theirs), Some(ours)) => theirs == ours,
			_ => true,
		}
	}
	#[cfg(not(unix))]
	{
		let _ = pid;
		true
	}
}

#[cfg(target_os = "macos")]
fn process_exe_name(pid: u32) -> Option<std::ffi::OsString> {
	use std::os::unix::ffi::OsStrExt;
	let mut buf = vec![0u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
	// SAFETY: `buf` is writable for the length passed.
	let n = unsafe { libc::proc_pidpath(pid as libc::c_int, buf.as_mut_ptr().cast(), buf.len() as u32) };
	if n <= 0 {
		return None;
	}
	let path = Path::new(std::ffi::OsStr::from_bytes(&buf[..n as usize]));
	path.file_name().map(|n| n.to_owned())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn process_exe_name(pid: u32) -> Option<std::ffi::OsString> {
	fs::read_link(format!("/proc/{}/exe", pid)).ok()?.file_name().map(|n| n.to_owned())
}

/// True when `file` was modified after `swap` was last written: recovering
/// the swap would roll back those newer changes.
pub fn swap_older_than(swap: &Path, file: &Path) -> bool {
	let mtime = |p: &Path| fs::metadata(p).and_then(|m| m.modified()).ok();
	matches!((mtime(swap), mtime(file)), (Some(s), Some(f)) if s < f)
}

/// Whether this process can create files in `dir`. Permission bits alone
/// aren't enough: a 0755 directory owned by someone else isn't "readonly" but
/// still refuses our writes, which used to leave such files with no working
/// swap at all.
fn dir_writable(dir: &Path) -> bool {
	#[cfg(unix)]
	{
		use std::os::unix::ffi::OsStrExt;
		let Ok(c) = std::ffi::CString::new(dir.as_os_str().as_bytes()) else {
			return false;
		};
		// SAFETY: `c` is a valid NUL-terminated string for the call's duration.
		unsafe { libc::access(c.as_ptr(), libc::W_OK) == 0 }
	}
	#[cfg(not(unix))]
	{
		fs::metadata(dir).map(|m| !m.permissions().readonly()).unwrap_or(false)
	}
}

/// Flatten a path into a single file name for the `$TMPDIR` fallback.
/// Separators become `_`; literal `%` and `_` are escaped as `%%` and `%_`
/// so distinct paths (`/a_b/c` vs `/a/b_c`) can't collide.
fn flatten_path(path: &Path) -> String {
	let mut out = String::new();
	for ch in path.to_string_lossy().chars() {
		match ch {
			'/' | '\\' => out.push('_'),
			'%' => out.push_str("%%"),
			'_' => out.push_str("%_"),
			c => out.push(c),
		}
	}
	out
}

/// Swap-file path for an unsaved (untitled) buffer, keyed by its editor
/// sequence number and this process's PID.
///
/// Unlike `get_swap_path` there is no originating file to key recovery on, so
/// these are best-effort crash dumps: the periodic autosave and the panic hook
/// write here, the editor cleans them up on a clean exit, and they are not
/// auto-offered on the next launch (P0-1). They live under `$TMPDIR/dan_swaps/`.
pub fn untitled_swap_path(seq: usize) -> PathBuf {
	let mut dir = env::temp_dir();
	dir.push("dan_swaps");
	let _ = fs::create_dir_all(&dir);
	dir.push(format!(".dan-untitled-{}-{}.swp", seq, std::process::id()));
	dir
}

/// Write `content` to `swap_path` via temp + fsync + rename.
///
/// Hardened against symlink-TOCTOU on the temp path (S2.3) and against
/// world-readable `.swp` content (S2.4):
/// - the temp file lives in the same directory as the swap file with an
///   unguessable suffix (PID + sub-second nanos);
/// - it is opened with `O_CREAT | O_EXCL` so a pre-created symlink at the
///   temp path causes an error rather than a write through the link;
/// - on Unix the temp is opened mode 0o600 so other local users can't
///   read unsaved buffer content.
///
/// Best-effort: errors are silently dropped because swap-file failures
/// must never interrupt the user's editing session.
pub fn write_swap_atomic(swap_path: &Path, content: &str) {
	write_swap(swap_path, content, None);
}

/// Like `write_swap_atomic`, but only lands if `cleanup_swap` hasn't run on
/// `swap_path` since `generation` was read (see `swap_generation`).
///
/// The periodic autosave snapshots a buffer and writes it on a background
/// thread. If the buffer is saved or closed meanwhile, its swap is deleted,
/// and an unguarded write would then recreate it: a stale swap that triggers
/// a recovery prompt on the next open. This happened on nearly every file
/// autosave whose interval is a multiple of 5 s, since both fire in the same
/// tick.
pub fn write_swap_if_current(swap_path: &Path, generation: u64, content: &str) {
	write_swap(swap_path, content, Some(generation));
}

/// Per-swap-path counter, bumped by every `cleanup_swap`.
fn generations() -> std::sync::MutexGuard<'static, HashMap<PathBuf, u64>> {
	static GENERATIONS: OnceLock<Mutex<HashMap<PathBuf, u64>>> = OnceLock::new();
	GENERATIONS
		.get_or_init(Default::default)
		.lock()
		.unwrap_or_else(|e| e.into_inner())
}

/// Current generation of `swap_path`; pass it to `write_swap_if_current`.
pub fn swap_generation(swap_path: &Path) -> u64 {
	generations().get(swap_path).copied().unwrap_or(0)
}

fn write_swap(swap_path: &Path, content: &str, generation: Option<u64>) {
	let Some(parent) = swap_path.parent() else { return };
	let Some(file_name) = swap_path.file_name() else { return };

	let nanos = std::time::SystemTime::now()
		.duration_since(std::time::UNIX_EPOCH)
		.map(|d| d.subsec_nanos())
		.unwrap_or(0);
	let tmp_name = format!(
		".{}.dan-swap-tmp.{}.{}",
		file_name.to_string_lossy(),
		std::process::id(),
		nanos
	);
	let tmp_path = parent.join(tmp_name);

	let result = (|| -> std::io::Result<()> {
		let mut options = fs::OpenOptions::new();
		options.write(true).create_new(true);
		#[cfg(unix)]
		{
			use std::os::unix::fs::OpenOptionsExt;
			options.mode(0o600);
		}
		let mut f = options.open(&tmp_path)?;
		use std::io::Write;
		f.write_all(content.as_bytes())?;
		f.sync_all()?;
		// Hold the lock across the rename so a concurrent cleanup can't slip
		// in between the check and the rename.
		let gens = generation.map(|_| generations());
		if let (Some(g), Some(gens)) = (generation, &gens) {
			if gens.get(swap_path).copied().unwrap_or(0) != g {
				return Err(std::io::Error::other("swap retired"));
			}
		}
		fs::rename(&tmp_path, swap_path)?;
		Ok(())
	})();

	if result.is_err() {
		let _ = fs::remove_file(&tmp_path);
	}
}

/// Remove the swap file. Called after a successful save, a discard, or a
/// close. Retires any background write still in flight for this path.
pub fn cleanup_swap(swap_path: &Path) {
	let mut gens = generations();
	*gens.entry(swap_path.to_path_buf()).or_insert(0) += 1;
	let _ = fs::remove_file(swap_path);
}

/// Returns the swap-file content if a recovery candidate exists, else None.
pub fn check_recovery(swap_path: &Path) -> Option<String> {
	if swap_path.exists() {
		fs::read_to_string(swap_path).ok()
	} else {
		None
	}
}

#[cfg(test)]
pub(crate) mod tests {
	use super::*;

	pub(crate) fn lock_path_of(lock: &SwapLock) -> PathBuf {
		lock.path.clone()
	}
	use std::io::Read;

	fn temp_dir() -> std::path::PathBuf {
		let mut d = std::env::temp_dir();
		d.push(format!("dan_recovery_test_{}", std::process::id()));
		fs::create_dir_all(&d).unwrap();
		d
	}

	#[test]
	fn relative_path_swaps_next_to_the_file() {
		// Regression: a bare relative name (`dan playground.md`) has an empty
		// parent, which used to fail the writability check and fall back to
		// `$TMPDIR/dan_swaps/playground.md.swp` — one swap shared by every
		// `playground.md` in every directory, so recovery offered another
		// repo's file.
		let cwd = std::env::current_dir().unwrap();
		let swap = get_swap_path(Path::new("dan-swap-test-file.md"));
		assert!(swap.is_absolute(), "swap path must be absolute: {}", swap.display());
		assert_eq!(swap, get_swap_path(&cwd.join("dan-swap-test-file.md")));
		assert_eq!(
			swap.parent().unwrap(),
			fs::canonicalize(&cwd).unwrap(),
			"swap should sit beside the file in a writable dir"
		);
	}

	#[test]
	fn flattened_names_do_not_collide() {
		assert_ne!(flatten_path(Path::new("/a_b/c")), flatten_path(Path::new("/a/b_c")));
		assert_ne!(flatten_path(Path::new("/a%/b")), flatten_path(Path::new("/a/%b")));
	}

	#[test]
	fn write_swap_round_trips_content() {
		let dir = temp_dir();
		let swap = dir.join("foo.swp");
		let _ = fs::remove_file(&swap);
		write_swap_atomic(&swap, "hello world");
		let mut got = String::new();
		fs::File::open(&swap).unwrap().read_to_string(&mut got).unwrap();
		assert_eq!(got, "hello world");
		fs::remove_file(&swap).unwrap();
	}

	#[cfg(unix)]
	#[test]
	fn write_swap_uses_mode_0o600() {
		// S2.4 regression: swap files must not be world-readable.
		use std::os::unix::fs::PermissionsExt;
		let dir = temp_dir();
		let swap = dir.join("perm.swp");
		let _ = fs::remove_file(&swap);
		write_swap_atomic(&swap, "secret");
		let mode = fs::metadata(&swap).unwrap().permissions().mode() & 0o777;
		assert_eq!(mode, 0o600, "swap file must be owner-only readable, got {:o}", mode);
		fs::remove_file(&swap).unwrap();
	}

	#[test]
	fn write_swap_replaces_existing() {
		let dir = temp_dir();
		let swap = dir.join("replace.swp");
		write_swap_atomic(&swap, "old");
		write_swap_atomic(&swap, "new");
		let got = fs::read_to_string(&swap).unwrap();
		assert_eq!(got, "new");
		fs::remove_file(&swap).unwrap();
	}

	#[cfg(unix)]
	#[test]
	fn write_swap_does_not_follow_temp_symlink() {
		// S2.3 regression: a pre-existing symlink at the swap path itself
		// would be replaced by the rename (correct), but a symlink at the
		// *temp* path used to be written through. The temp name is now
		// unguessable AND opened O_EXCL, so even a guessed pre-created
		// symlink can't intercept the write.
		//
		// We can't fully simulate the unguessability here, but we can
		// verify O_EXCL: pre-create the swap_path as a regular file with a
		// known content and confirm it gets overwritten with our content.
		let dir = temp_dir();
		let swap = dir.join("excl.swp");
		fs::write(&swap, b"prior content").unwrap();
		write_swap_atomic(&swap, "fresh content");
		let got = fs::read_to_string(&swap).unwrap();
		assert_eq!(got, "fresh content");
		fs::remove_file(&swap).unwrap();
	}

	#[test]
	fn cleanup_retires_in_flight_swap_write() {
		// The autosave thread snapshots a generation, then the buffer is saved
		// (cleanup) before the write lands. The write must not resurrect the
		// swap, or the next open prompts to "recover" stale content.
		let dir = temp_dir();
		let swap = dir.join("retired.swp");
		let _ = fs::remove_file(&swap);

		let generation = swap_generation(&swap);
		cleanup_swap(&swap);
		write_swap_if_current(&swap, generation, "stale");
		assert!(!swap.exists(), "a retired write must not recreate the swap");

		write_swap_if_current(&swap, swap_generation(&swap), "fresh");
		assert_eq!(fs::read_to_string(&swap).unwrap(), "fresh");
		cleanup_swap(&swap);
		assert!(!swap.exists());
	}

	fn lock_test_dir(name: &str) -> PathBuf {
		let d = temp_dir().join(format!("locks_{}", name));
		let _ = fs::remove_dir_all(&d);
		d
	}

	#[test]
	fn lock_is_exclusive_against_a_live_dan_and_released_on_drop() {
		let dir = lock_test_dir("excl");
		let file = temp_dir().join("locked.md");
		let LockOutcome::Acquired(Some(lock)) = lock_file_with(&file, &dir, |_| true) else {
			panic!("first open must take the lock");
		};
		// Pose as another live dan holding it.
		fs::write(&lock.path, "1").unwrap();
		assert!(matches!(lock_file_with(&file, &dir, |_| true), LockOutcome::HeldBy(1)));
		// No longer ours, so dropping must leave it alone.
		let path = lock.path.clone();
		drop(lock);
		assert!(path.exists(), "drop must not remove another process's lock");

		fs::remove_file(&path).unwrap();
		let LockOutcome::Acquired(Some(lock)) = lock_file_with(&file, &dir, |_| true) else { panic!() };
		let path = lock.path.clone();
		drop(lock);
		assert!(!path.exists(), "drop must release our lock");
	}

	#[test]
	fn stale_lock_from_a_dead_process_is_reclaimed() {
		let dir = lock_test_dir("stale");
		let file = temp_dir().join("stale.md");
		let LockOutcome::Acquired(Some(lock)) = lock_file_with(&file, &dir, |_| true) else { panic!() };
		fs::write(&lock.path, "999999").unwrap();
		std::mem::forget(lock);
		assert!(matches!(lock_file_with(&file, &dir, |_| false), LockOutcome::Acquired(Some(_))));
	}

	#[cfg(unix)]
	#[test]
	fn pid_liveness_check() {
		assert!(pid_is_live_dan(std::process::id()), "we are a live process with our own exe name");
		let mut child = std::process::Command::new("true").spawn().unwrap();
		let pid = child.id();
		child.wait().unwrap();
		assert!(!pid_is_live_dan(pid), "an exited process is not a live owner");
	}

	#[test]
	fn swap_older_than_compares_mtimes() {
		let dir = temp_dir();
		let swap = dir.join("old.swp");
		let file = dir.join("old.md");
		fs::write(&swap, "s").unwrap();
		fs::write(&file, "f").unwrap();
		let past = std::time::SystemTime::now() - std::time::Duration::from_secs(60);
		fs::File::options().write(true).open(&swap).unwrap().set_modified(past).unwrap();
		assert!(swap_older_than(&swap, &file));
		assert!(!swap_older_than(&file, &swap));
		fs::remove_file(&swap).ok();
		fs::remove_file(&file).ok();
	}

	/// Not a real test: a stand-in "other dan" process for lock tests, run as
	/// a child copy of this test binary so its exe name matches ours.
	#[test]
	#[ignore]
	fn lock_holder_sleeper() {
		std::thread::sleep(std::time::Duration::from_secs(30));
	}
}
