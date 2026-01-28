//! Build script to inject build-time information into the binary.
//!
//! This sets environment variables that can be read at compile time:
//! - `BN_BUILD_TIMESTAMP`: ISO 8601 timestamp when the binary was built
//! - `BN_GIT_COMMIT`: Short git commit hash (or "unknown" if not in a git repo)
//! - `BN_COPILOT_VERSION`: Version from COPILOT_VERSION file (or "unknown" if not found)
//! - `BN_WEB_BUNDLE`: Path to embedded web bundle (when gui feature enabled)

use std::process::Command;

fn main() {
    // Rerun if git HEAD changes (new commit)
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/index");

    // Rerun if COPILOT_VERSION changes
    println!("cargo:rerun-if-changed=COPILOT_VERSION");

    // Get build timestamp
    let timestamp = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    println!("cargo:rustc-env=BN_BUILD_TIMESTAMP={}", timestamp);

    // Get git commit hash
    let commit = get_git_commit().unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=BN_GIT_COMMIT={}", commit);

    // Get Copilot version
    let copilot_version = get_copilot_version().unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=BN_COPILOT_VERSION={}", copilot_version);

    // Bundle and embed web assets when gui feature is enabled
    #[cfg(feature = "gui")]
    bundle_web_assets();
}

#[cfg(feature = "gui")]
fn bundle_web_assets() {
    use std::fs;
    use std::path::Path;

    // Rerun if web files change
    println!("cargo:rerun-if-changed=web/");
    println!("cargo:rerun-if-changed=scripts/bundle-web.sh");

    let bundle_path = Path::new("target/web-bundle.tar.zst");
    let hash_file = Path::new("target/web-bundle.hash");

    // Compute current hash of web/ directory
    let current_hash = hash_directory("web").unwrap_or_else(|e| {
        eprintln!("Warning: Failed to hash web/ directory: {}", e);
        0
    });

    // Check if we need to rebuild
    let needs_rebuild = !bundle_path.exists() || !hash_file.exists() || {
        let cached_hash = fs::read_to_string(hash_file)
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok())
            .unwrap_or(0);
        cached_hash != current_hash
    };

    if needs_rebuild {
        eprintln!("Building web bundle...");
        let status = Command::new("./scripts/bundle-web.sh")
            .status()
            .expect("Failed to run bundle-web.sh");

        if !status.success() {
            panic!("Web bundle script failed");
        }

        // Save hash for next build
        if let Err(e) = fs::write(hash_file, current_hash.to_string()) {
            eprintln!("Warning: Failed to save bundle hash: {}", e);
        }
    } else {
        eprintln!("Web bundle up to date (cached)");
    }

    // Expose bundle path for include_bytes! in code
    println!("cargo:rustc-env=BN_WEB_BUNDLE={}", bundle_path.display());
}

/// Hash all files in a directory recursively
#[cfg(feature = "gui")]
fn hash_directory(path: impl AsRef<std::path::Path>) -> std::io::Result<u64> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::Hasher;

    let mut hasher = DefaultHasher::new();
    hash_directory_recursive(path.as_ref(), &mut hasher)?;
    Ok(hasher.finish())
}

#[cfg(feature = "gui")]
fn hash_directory_recursive(
    path: &std::path::Path,
    hasher: &mut std::collections::hash_map::DefaultHasher,
) -> std::io::Result<()> {
    use std::fs;
    use std::hash::Hash;
    if !path.exists() {
        return Ok(());
    }

    if path.is_file() {
        // Hash file path and contents
        path.to_string_lossy().hash(hasher);
        let metadata = fs::metadata(path)?;
        metadata.len().hash(hasher);
        if let Ok(modified) = metadata.modified()
            && let Ok(duration) = modified.duration_since(std::time::UNIX_EPOCH)
        {
            duration.as_secs().hash(hasher);
        }
    } else if path.is_dir() {
        // Hash directory entries in sorted order for consistency
        let mut entries: Vec<_> = fs::read_dir(path)?.collect::<Result<_, _>>()?;
        entries.sort_by_key(|e| e.path());

        for entry in entries {
            let entry_path = entry.path();
            // Skip test files and hidden files
            if let Some(name) = entry_path.file_name().and_then(|n| n.to_str())
                && (name.ends_with(".test.js") || name.starts_with('.'))
            {
                continue;
            }
            hash_directory_recursive(&entry_path, hasher)?;
        }
    }

    Ok(())
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

fn get_copilot_version() -> Option<String> {
    use std::fs;
    let contents = fs::read_to_string("COPILOT_VERSION").ok()?;
    // Parse the version - expected format is "v0.0.396" on line 4
    // Read all lines and find the one that starts with 'v'
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('v') && !trimmed.starts_with('#') {
            return Some(trimmed.to_string());
        }
    }
    None
}
