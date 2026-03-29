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

	// Automatically trigger rebuilds natively resolving git metadata states
	// Re-run if HEAD changes (e.g. branch switch)
	println!("cargo:rerun-if-changed=.git/HEAD");
	// Re-run if index changes (e.g. commits)
	println!("cargo:rerun-if-changed=.git/index");
	// Re-run if branch head history changes natively handling any branch commit globally
	println!("cargo:rerun-if-changed=.git/logs/HEAD");
	println!("cargo:rerun-if-changed=.git/refs/");
	
	// Read HEAD to explicitly bind the exact branch object file tracking locally natively fallback
	if let Ok(head_content) = std::fs::read_to_string(".git/HEAD") {
		if let Some(ref_path) = head_content.strip_prefix("ref: ") {
			println!("cargo:rerun-if-changed=.git/{}", ref_path.trim());
		}
	}
	
	println!("cargo:rerun-if-changed=VERSION");
}
