//! Build script to inject build-time information into the binary.
//!
//! This sets environment variables that can be read at compile time:
//! - `BN_BUILD_TIMESTAMP`: ISO 8601 timestamp when the binary was built
//! - `BN_GIT_COMMIT`: Short git commit hash (or "unknown" if not in a git repo)
//! - `BN_WEB_BUNDLE`: Path to embedded web bundle (when gui feature enabled)

use std::path::Path;
use std::process::Command;

fn main() {
    // Rerun if git HEAD changes (new commit)
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/index");

    // Get build timestamp
    let timestamp = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    println!("cargo:rustc-env=BN_BUILD_TIMESTAMP={}", timestamp);

    // Get git commit hash
    let commit = get_git_commit().unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=BN_GIT_COMMIT={}", commit);

    // Bundle and embed web assets when gui feature is enabled
    #[cfg(feature = "gui")]
    bundle_web_assets();
}

#[cfg(feature = "gui")]
fn bundle_web_assets() {
    // Rerun if web files change
    println!("cargo:rerun-if-changed=web/");
    println!("cargo:rerun-if-changed=scripts/bundle-web.sh");

    let bundle_path = Path::new("target/web-bundle.tar.zst");

    // Run bundle script if bundle doesn't exist or is stale
    if !bundle_path.exists() {
        eprintln!("Building web bundle...");
        let status = Command::new("./scripts/bundle-web.sh")
            .status()
            .expect("Failed to run bundle-web.sh");

        if !status.success() {
            panic!("Web bundle script failed");
        }
    }

    // Expose bundle path for include_bytes! in code
    println!("cargo:rustc-env=BN_WEB_BUNDLE={}", bundle_path.display());
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
