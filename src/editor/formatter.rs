use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::mpsc;
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
			"js" | "jsx" | "ts" | "tsx" | "json" | "css" | "scss" | "html" | "md" | "yaml" | "yml" => Tool::Prettier,
			_ => Tool::Unknown(ext.to_string()),
		}
	}
}

/// Spawns an external unblocking structural thread evaluating specific language payloads safely reporting exclusively over an MPSC boundary.
pub fn spawn_formatter(
	ext_str: String,
	content: String,
	tx: mpsc::Sender<Result<String, String>>,
) {
	thread::spawn(move || {
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
				if let Some(mut stdin) = c.stdin.take() {
					let _ = stdin.write_all(content.as_bytes());
				}
				match c.wait_with_output() {
					Ok(output) => {
						if output.status.success() {
							let _ = tx.send(Ok(String::from_utf8_lossy(&output.stdout).to_string()));
						} else {
							let err_str = String::from_utf8_lossy(&output.stderr);
							let first_line = err_str.lines().next().unwrap_or("Formatter syntax error");
							let _ = tx.send(Err(first_line.to_string()));
						}
					}
					Err(e) => {
						let _ = tx.send(Err(format!("Formatter failed to wait: {}", e)));
					}
				}
			}
			Err(e) => {
				if e.kind() == std::io::ErrorKind::NotFound {
					let binary_name = match tool {
						Tool::Prettier => "prettier",
						Tool::Rustfmt => "rustfmt",
						Tool::Ruff => "ruff",
						_ => "formatter",
					};
					let _ = tx.send(Err(format!("Formatter '{}' not found in $PATH", binary_name)));
				} else {
					let _ = tx.send(Err(format!("Error spawning formatter: {}", e)));
				}
			}
		}
	});
}
