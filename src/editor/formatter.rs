use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;

pub enum Tool {
	Prettier,
	Rustfmt,
	Ruff,
	Unknown(String),
}

impl Tool {
	pub fn from_extension(ext: &str) -> Self {
		match ext {
			"rs" => Tool::Rustfmt,
			"py" => Tool::Ruff,
			"js" | "jsx" | "ts" | "tsx" | "json" | "css" | "scss" | "html" | "md" | "yaml"
			| "yml" => Tool::Prettier,
			_ => Tool::Unknown(ext.to_string()),
		}
	}
}

/// Spawns an external unblocking structural thread evaluating specific language payloads safely reporting exclusively over an MPSC boundary.
///
/// `child_pid` is set to the spawned formatter child's PID so the editor can
/// terminate it on shutdown and avoid orphaned prettier/rustfmt/ruff processes.
pub fn spawn_formatter(
	ext_str: String,
	content: String,
	tx: mpsc::Sender<Result<String, String>>,
	child_pid: Arc<AtomicU32>,
) {
	thread::spawn(move || {
		let clear_pid = || child_pid.store(0, Ordering::Relaxed);
		let tool = Tool::from_extension(&ext_str);

		let child = match &tool {
			Tool::Prettier => Command::new("prettier")
				.arg("--stdin-filepath")
				.arg(format!("file.{}", ext_str))
				.stdin(Stdio::piped())
				.stdout(Stdio::piped())
				.stderr(Stdio::piped())
				.spawn(),
			Tool::Rustfmt => Command::new("rustfmt")
				.arg("--edition=2021")
				.stdin(Stdio::piped())
				.stdout(Stdio::piped())
				.stderr(Stdio::piped())
				.spawn(),
			Tool::Ruff => Command::new("ruff")
				.arg("format")
				.arg("-")
				.stdin(Stdio::piped())
				.stdout(Stdio::piped())
				.stderr(Stdio::piped())
				.spawn(),
			Tool::Unknown(ext) => {
				let _ = tx.send(Err(format!("Formatter not supported for .{} files", ext)));
				return;
			}
		};

		match child {
			Ok(mut c) => {
				child_pid.store(c.id(), Ordering::Relaxed);
				// Write stdin on a dedicated thread so wait_with_output can drain
				// stdout/stderr concurrently. Writing the whole input before
				// reading any output deadlocks once the child fills its
				// stdout/stderr pipe (~64 KB) on a large file (P2-E).
				if let Some(mut stdin) = c.stdin.take() {
					let bytes = content.into_bytes();
					thread::spawn(move || {
						let _ = stdin.write_all(&bytes);
						// stdin dropped here → EOF for the child.
					});
				}
				match c.wait_with_output() {
					Ok(output) => {
						clear_pid();
						if output.status.success() {
							let _ =
								tx.send(Ok(String::from_utf8_lossy(&output.stdout).to_string()));
						} else {
							let err_str = String::from_utf8_lossy(&output.stderr);
							let first_line =
								err_str.lines().next().unwrap_or("Formatter syntax error");
							let _ = tx.send(Err(first_line.to_string()));
						}
					}
					Err(e) => {
						clear_pid();
						let _ = tx.send(Err(format!("Formatter failed to wait: {}", e)));
					}
				}
			}
			Err(e) => {
				clear_pid();
				if e.kind() == std::io::ErrorKind::NotFound {
					let binary_name = match tool {
						Tool::Prettier => "prettier",
						Tool::Rustfmt => "rustfmt",
						Tool::Ruff => "ruff",
						_ => "formatter",
					};
					let _ = tx.send(Err(format!(
						"Formatter '{}' not found in $PATH",
						binary_name
					)));
				} else {
					let _ = tx.send(Err(format!("Error spawning formatter: {}", e)));
				}
			}
		}
	});
}

/// Send SIGTERM to a formatter child process. No-op for pid 0 or on non-Unix.
pub fn terminate_child(pid: u32) {
	if pid == 0 {
		return;
	}
	#[cfg(unix)]
	{
		let _ = std::process::Command::new("kill")
			.arg(pid.to_string())
			.stdin(Stdio::null())
			.stdout(Stdio::null())
			.stderr(Stdio::null())
			.status();
	}
}
