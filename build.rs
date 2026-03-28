/// Build script — embeds the short git hash as `GIT_HASH` env var.
use std::process::Command;

fn main() {
	// Get short git hash (7 chars)
	let output = Command::new("git")
		.args(["rev-parse", "--short", "HEAD"])
		.output();

	let hash = match output {
		Ok(o) if o.status.success() => {
			String::from_utf8_lossy(&o.stdout).trim().to_string()
		}
		_ => "unknown".to_string(),
	};

	println!("cargo:rustc-env=GIT_HASH={}", hash);

	// Re-run if HEAD changes (branch switch)
	println!("cargo:rerun-if-changed=.git/HEAD");
	
	// Read HEAD to determine the actual branch ref file to watch (for new commits)
	if let Ok(head_content) = std::fs::read_to_string(".git/HEAD") {
		if let Some(ref_path) = head_content.strip_prefix("ref: ") {
			println!("cargo:rerun-if-changed=.git/{}", ref_path.trim());
		}
	}
	
	println!("cargo:rerun-if-changed=VERSION");
}
