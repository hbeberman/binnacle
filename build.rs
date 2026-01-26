//! Build script to inject build-time information into the binary.
//!
//! This sets environment variables that can be read at compile time:
//! - `BN_BUILD_TIMESTAMP`: ISO 8601 timestamp when the binary was built
//! - `BN_GIT_COMMIT`: Short git commit hash (or "unknown" if not in a git repo)

use std::process::Command;

fn main() {
    // Rerun if git HEAD changes (new commit)
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/index");

    // Rerun if GUI files change
    println!("cargo:rerun-if-changed=src/gui/index.html");

    // Get build timestamp
    let timestamp = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    println!("cargo:rustc-env=BN_BUILD_TIMESTAMP={}", timestamp);

    // Get git commit hash
    let commit = get_git_commit().unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=BN_GIT_COMMIT={}", commit);
}

fn get_git_commit() -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()?;

    if output.status.success() {
        let hash = String::from_utf8(output.stdout).ok()?;
        Some(hash.trim().to_string())
    } else {
        None
    }
}
