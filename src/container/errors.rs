//! Standardized error messages for container operations.
//!
//! All container errors follow the format:
//! ```text
//! bn: error: <category>: <brief>
//!
//!   <details>
//!
//!   <suggestion>
//! ```
//!
//! This module provides helper functions to format errors consistently
//! according to the error catalog in PRD bn-ec60.

use std::fmt::Write;

/// Error category for container operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCategory {
    /// Configuration errors (KDL parsing, schema, validation)
    Config,
    /// Build errors (Containerfile, parent resolution)
    Build,
    /// Runtime errors (mounts, image availability)
    Run,
    /// Mount-specific errors
    Mount,
    /// Conflict errors (ambiguous definitions)
    Conflict,
}

impl std::fmt::Display for ErrorCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Config => write!(f, "config"),
            Self::Build => write!(f, "build"),
            Self::Run => write!(f, "run"),
            Self::Mount => write!(f, "mount"),
            Self::Conflict => write!(f, "conflict"),
        }
    }
}

/// Format a standardized error message.
///
/// # Arguments
/// * `category` - The error category
/// * `brief` - A brief description of the error
/// * `details` - Optional detailed context (will be indented)
/// * `suggestion` - Optional suggestion for fixing the error (will be indented)
///
/// # Returns
/// A formatted error message string
pub fn format_error(
    category: ErrorCategory,
    brief: &str,
    details: Option<&str>,
    suggestion: Option<&str>,
) -> String {
    let mut msg = format!("bn: error: {}: {}", category, brief);

    if let Some(details) = details {
        msg.push_str("\n\n");
        // Indent each line of details
        for line in details.lines() {
            let _ = writeln!(msg, "  {}", line);
        }
        // Remove trailing newline from details block
        if msg.ends_with('\n') {
            msg.pop();
        }
    }

    if let Some(suggestion) = suggestion {
        msg.push_str("\n\n");
        // Indent suggestion
        for line in suggestion.lines() {
            let _ = writeln!(msg, "  {}", line);
        }
        // Remove trailing newline from suggestion block
        if msg.ends_with('\n') {
            msg.pop();
        }
    }

    msg
}

/// Format a standardized warning message.
///
/// # Arguments
/// * `category` - The error category
/// * `brief` - A brief description of the warning
/// * `details` - Optional detailed context
///
/// # Returns
/// A formatted warning message string
pub fn format_warning(category: ErrorCategory, brief: &str, details: Option<&str>) -> String {
    let mut msg = format!("bn: warning: {}: {}", category, brief);

    if let Some(details) = details {
        msg.push_str("\n\n");
        for line in details.lines() {
            let _ = writeln!(msg, "  {}", line);
        }
        if msg.ends_with('\n') {
            msg.pop();
        }
    }

    msg
}

// ============================================================================
// Error catalog - specific error constructors
// ============================================================================

/// Error: Invalid KDL syntax
pub fn invalid_kdl_syntax(line: Option<usize>, context: &str) -> String {
    let brief = if let Some(line) = line {
        format!("invalid KDL syntax at line {}", line)
    } else {
        "invalid KDL syntax".to_string()
    };

    format_error(
        ErrorCategory::Config,
        &brief,
        Some(context),
        Some("Check for missing quotes, braces, or invalid characters."),
    )
}

/// Error: Reserved container name
pub fn reserved_name(name: &str) -> String {
    format_error(
        ErrorCategory::Config,
        "reserved container name",
        Some(&format!(
            "Container name '{}' is reserved for internal use.",
            name
        )),
        Some(
            "Choose a different name for your container definition (e.g., 'worker', 'dev', 'agent').",
        ),
    )
}

/// Error: Duplicate container name
pub fn duplicate_name(name: &str, first_line: Option<usize>, second_line: Option<usize>) -> String {
    let mut details = format!("Container '{}' is defined twice", name);
    if let (Some(first), Some(second)) = (first_line, second_line) {
        details.push_str(&format!(
            "\n  - First: line {}\n  - Second: line {}",
            first, second
        ));
    }

    format_error(
        ErrorCategory::Config,
        "duplicate container name",
        Some(&details),
        Some("Each container name must be unique."),
    )
}

/// Error: Circular dependency detected
pub fn circular_dependency(cycle: &[&str]) -> String {
    let chain = cycle.join(" → ");
    format_error(
        ErrorCategory::Config,
        "circular dependency detected",
        Some(&format!("Dependency chain forms a cycle:\n  {}", chain)),
        Some("Remove one parent relationship to break the cycle."),
    )
}

/// Error: Invalid container name characters
pub fn invalid_name_characters(name: &str) -> String {
    format_error(
        ErrorCategory::Config,
        "invalid container name",
        Some(&format!(
            "Container name '{}' contains invalid characters.",
            name
        )),
        Some("Use only alphanumeric characters, hyphens, and underscores."),
    )
}

/// Error: Empty container name
pub fn empty_name() -> String {
    format_error(
        ErrorCategory::Config,
        "empty container name",
        Some("Container name cannot be empty."),
        Some("Provide a non-empty name for the container."),
    )
}

/// Error: Name mismatch between key and definition
pub fn name_mismatch(key: &str, def_name: &str) -> String {
    format_error(
        ErrorCategory::Config,
        "container name mismatch",
        Some(&format!(
            "Key '{}' does not match definition name '{}'.",
            key, def_name
        )),
        Some("Ensure the container key matches the name in its definition."),
    )
}

/// Error: Mount target must be absolute path
pub fn mount_target_not_absolute(container: &str, mount_name: &str, target: &str) -> String {
    format_error(
        ErrorCategory::Config,
        "mount target must be absolute path",
        Some(&format!(
            "Container '{}': mount '{}' has relative target '{}'.",
            container, mount_name, target
        )),
        Some("Use an absolute path starting with '/' for the mount target."),
    )
}

/// Error: Container node must have a name
pub fn missing_container_name() -> String {
    format_error(
        ErrorCategory::Config,
        "missing container name",
        Some("Container node must have a name argument."),
        Some("Add a name: container \"my-container\" { ... }"),
    )
}

/// Error: Mount node must have a name
pub fn missing_mount_name() -> String {
    format_error(
        ErrorCategory::Config,
        "missing mount name",
        Some("Mount node must have a name argument."),
        Some("Add a name: mount \"data\" target=\"/data\""),
    )
}

/// Error: Mount must have a target
pub fn missing_mount_target(mount_name: &str) -> String {
    format_error(
        ErrorCategory::Config,
        "missing mount target",
        Some(&format!("Mount '{}' must have a target.", mount_name)),
        Some("Add a target: mount \"data\" target=\"/data\""),
    )
}

/// Error: Invalid mount mode
pub fn invalid_mount_mode(mode: &str) -> String {
    format_error(
        ErrorCategory::Config,
        "invalid mount mode",
        Some(&format!("Unknown mount mode: '{}'.", mode)),
        Some("Valid modes are: 'ro' (read-only), 'rw' (read-write)."),
    )
}

/// Error: Missing parent container
pub fn missing_parent(container: &str, parent: &str, searched_paths: &[&str]) -> String {
    let mut details = format!(
        "Container '{}' requires parent '{}', but '{}' is not defined.",
        container, parent, parent
    );

    if !searched_paths.is_empty() {
        details.push_str("\n\nSearched:");
        for path in searched_paths {
            details.push_str(&format!("\n  - {}", path));
        }
    }

    format_error(
        ErrorCategory::Build,
        "parent container not found",
        Some(&details),
        Some(&format!(
            "Create the parent '{}' or remove 'parent \"{}\"'.",
            parent, parent
        )),
    )
}

/// Error: Missing Containerfile
pub fn missing_containerfile(container: &str, path: &str) -> String {
    format_error(
        ErrorCategory::Build,
        "Containerfile not found",
        Some(&format!(
            "Container '{}' has no Containerfile at:\n  {}",
            container, path
        )),
        Some("Create the Containerfile or check the directory name."),
    )
}

/// Error: Mount source path not found (required mount)
pub fn mount_source_not_found(mount_name: &str, source_path: &str) -> String {
    format_error(
        ErrorCategory::Mount,
        "source path not found",
        Some(&format!("Mount '{}' requires: {}", mount_name, source_path)),
        Some(&format!(
            "Options:\n  1. Create: mkdir -p {}\n  2. Add optional=true in config.kdl",
            source_path
        )),
    )
}

/// Warning: Optional mount source not found
pub fn optional_mount_skipped(mount_name: &str, source_path: &str) -> String {
    format_warning(
        ErrorCategory::Mount,
        &format!("skipping optional mount '{}'", mount_name),
        Some(&format!(
            "Source not found: {}\nContainer will start without this mount.",
            source_path
        )),
    )
}

/// Warning: Mount source may not exist (build-time check)
pub fn mount_source_may_not_exist(mount_name: &str, source_path: &str) -> String {
    format_warning(
        ErrorCategory::Build,
        "mount source may not exist",
        Some(&format!(
            "Mount '{}' references: {}\nPath does not exist on this machine.",
            mount_name, source_path
        )),
    )
}

/// Error: Image not found
pub fn image_not_found(image: &str, container: &str) -> String {
    format_error(
        ErrorCategory::Run,
        "container image not found",
        Some(&format!("Image '{}' not found.", image)),
        Some(&format!("Build first: bn container build {}", container)),
    )
}

/// Error: Ambiguous container definition (conflict)
pub fn ambiguous_definition(
    name: &str,
    project_path: &str,
    project_desc: Option<&str>,
    project_modified: Option<&str>,
    host_path: &str,
    host_desc: Option<&str>,
    host_modified: Option<&str>,
) -> String {
    let mut details = format!(
        "Container '{}' exists in multiple locations:\n\n  --project  {}",
        name, project_path
    );
    if let Some(modified) = project_modified {
        details.push_str(&format!("\n             Modified: {}", modified));
    }
    if let Some(desc) = project_desc {
        details.push_str(&format!("\n             Description: \"{}\"", desc));
    }

    details.push_str(&format!("\n\n  --host     {}", host_path));
    if let Some(modified) = host_modified {
        details.push_str(&format!("\n             Modified: {}", modified));
    }
    if let Some(desc) = host_desc {
        details.push_str(&format!("\n             Description: \"{}\"", desc));
    }

    format_error(
        ErrorCategory::Conflict,
        "ambiguous container definition",
        Some(&details),
        Some("Re-run with --project or --host to specify which definition to use."),
    )
}

/// Error: Cannot expand home directory
pub fn home_expansion_failed(path: &str) -> String {
    format_error(
        ErrorCategory::Config,
        "cannot expand home directory",
        Some(&format!(
            "Cannot expand '{}' because home directory is not set.",
            path
        )),
        Some("Ensure $HOME environment variable is set, or use an absolute path."),
    )
}

/// Error: Failed to resolve mount source
pub fn mount_resolve_failed(mount_name: &str, source: &str, error: &str) -> String {
    format_error(
        ErrorCategory::Mount,
        "failed to resolve mount source",
        Some(&format!(
            "Mount '{}' source '{}' could not be resolved:\n  {}",
            mount_name, source, error
        )),
        Some("Check that the path exists and is accessible."),
    )
}

/// Error: Definition not found
pub fn definition_not_found(name: &str, searched_paths: &[&str]) -> String {
    let mut details = format!("Container definition '{}' was not found.", name);

    if !searched_paths.is_empty() {
        details.push_str("\n\nSearched:");
        for path in searched_paths {
            details.push_str(&format!("\n  - {}", path));
        }
    }

    format_error(
        ErrorCategory::Config,
        "definition not found",
        Some(&details),
        Some(&format!(
            "Create a definition for '{}' in .binnacle/containers/config.kdl",
            name
        )),
    )
}

/// Error: Failed to parse config file
pub fn config_parse_failed(path: &str, error: &str) -> String {
    format_error(
        ErrorCategory::Config,
        "failed to parse config",
        Some(&format!("Failed to parse {}:\n  {}", path, error)),
        Some("Check the KDL syntax in your config file."),
    )
}

/// Error: Failed to read config file
pub fn config_read_failed(path: &str, error: &str) -> String {
    format_error(
        ErrorCategory::Config,
        "failed to read config",
        Some(&format!("Failed to read {}:\n  {}", path, error)),
        Some("Check that the file exists and is readable."),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_error_basic() {
        let msg = format_error(ErrorCategory::Config, "test brief", None, None);
        assert_eq!(msg, "bn: error: config: test brief");
    }

    #[test]
    fn test_format_error_with_details() {
        let msg = format_error(
            ErrorCategory::Config,
            "test brief",
            Some("detail line 1\ndetail line 2"),
            None,
        );
        assert!(msg.contains("bn: error: config: test brief"));
        assert!(msg.contains("  detail line 1"));
        assert!(msg.contains("  detail line 2"));
    }

    #[test]
    fn test_format_error_with_suggestion() {
        let msg = format_error(
            ErrorCategory::Config,
            "test brief",
            Some("details"),
            Some("suggestion"),
        );
        assert!(msg.contains("bn: error: config: test brief"));
        assert!(msg.contains("  details"));
        assert!(msg.contains("  suggestion"));
    }

    #[test]
    fn test_reserved_name_error() {
        let msg = reserved_name("binnacle");
        assert!(msg.contains("bn: error: config: reserved container name"));
        assert!(msg.contains("'binnacle' is reserved"));
        assert!(msg.contains("Choose a different name"));
    }

    #[test]
    fn test_circular_dependency_error() {
        let msg = circular_dependency(&["worker", "base", "common", "worker"]);
        assert!(msg.contains("bn: error: config: circular dependency detected"));
        assert!(msg.contains("worker → base → common → worker"));
        assert!(msg.contains("Remove one parent relationship"));
    }

    #[test]
    fn test_mount_source_not_found_error() {
        let msg = mount_source_not_found("datasets", "/data/shared");
        assert!(msg.contains("bn: error: mount: source path not found"));
        assert!(msg.contains("Mount 'datasets' requires: /data/shared"));
        assert!(msg.contains("mkdir -p /data/shared"));
        assert!(msg.contains("optional=true"));
    }

    #[test]
    fn test_format_warning() {
        let msg = format_warning(
            ErrorCategory::Build,
            "mount source may not exist",
            Some("Path does not exist"),
        );
        assert!(msg.contains("bn: warning: build: mount source may not exist"));
        assert!(msg.contains("  Path does not exist"));
    }

    #[test]
    fn test_ambiguous_definition_error() {
        let msg = ambiguous_definition(
            "rust-dev",
            ".binnacle/containers/rust-dev/",
            Some("Project-specific Rust 1.75"),
            Some("2026-01-28 14:30"),
            "~/.local/share/binnacle/a1b2c3/containers/rust-dev/",
            Some("My standard Rust env"),
            Some("2026-01-15 09:15"),
        );
        assert!(msg.contains("bn: error: conflict: ambiguous container definition"));
        assert!(msg.contains("--project"));
        assert!(msg.contains("--host"));
        assert!(msg.contains("Project-specific Rust 1.75"));
    }

    #[test]
    fn test_missing_parent_error() {
        let msg = missing_parent(
            "rust-dev",
            "base",
            &[
                ".binnacle/containers/base/",
                "~/.local/share/binnacle/a1b2c3/containers/base/",
            ],
        );
        assert!(msg.contains("bn: error: build: parent container not found"));
        assert!(msg.contains("requires parent 'base'"));
        assert!(msg.contains(".binnacle/containers/base/"));
    }
}
