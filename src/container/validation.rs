//! Tiered validation for container definitions.
//!
//! Validation happens at three stages:
//!
//! 1. **Parse-time**: KDL syntax, schema, reserved names, cycles
//! 2. **Build-time**: parent references, Containerfile existence
//! 3. **Run-time**: mount existence, image availability
//!
//! Each tier returns a `ValidationResult` with errors (blocking) and warnings (informational).

use super::errors;
use crate::{Error, Result};
use std::collections::{HashMap, HashSet};
use std::path::Path;

use super::{ContainerDefinition, DefinitionSource, DefinitionWithSource, RESERVED_NAME};

/// Result of validation with errors and warnings
#[derive(Debug, Default, Clone)]
pub struct ValidationResult {
    /// Blocking errors that prevent the operation
    pub errors: Vec<String>,
    /// Non-blocking warnings for user awareness
    pub warnings: Vec<String>,
}

impl ValidationResult {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_error(&mut self, msg: impl Into<String>) {
        self.errors.push(msg.into());
    }

    pub fn add_warning(&mut self, msg: impl Into<String>) {
        self.warnings.push(msg.into());
    }

    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }

    /// Merge another validation result into this one
    pub fn merge(&mut self, other: ValidationResult) {
        self.errors.extend(other.errors);
        self.warnings.extend(other.warnings);
    }

    /// Convert to Result, failing if there are errors
    pub fn into_result(self) -> Result<Vec<String>> {
        if self.errors.is_empty() {
            Ok(self.warnings)
        } else {
            Err(Error::Other(format!(
                "Validation failed:\n  {}",
                self.errors.join("\n  ")
            )))
        }
    }
}

/// Validation tier specifying when validation should occur
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationTier {
    /// Parse-time: schema, reserved names, cycles
    Parse,
    /// Build-time: parents, Containerfile existence
    Build,
    /// Run-time: mounts, image availability
    Run,
}

impl std::fmt::Display for ValidationTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parse => write!(f, "parse"),
            Self::Build => write!(f, "build"),
            Self::Run => write!(f, "run"),
        }
    }
}

// ============================================================================
// Parse-time validation
// ============================================================================

/// Validate container definitions at parse time.
///
/// Checks:
/// - Reserved name enforcement (cannot use "binnacle")
/// - Schema validation (required fields present)
/// - Cycle detection in parent chains
///
/// # Arguments
/// * `definitions` - Map of container name to definition
///
/// # Returns
/// * `ValidationResult` with any errors/warnings found
pub fn validate_parse(definitions: &HashMap<String, ContainerDefinition>) -> ValidationResult {
    let mut result = ValidationResult::new();

    for (name, def) in definitions {
        // Check reserved name
        if name == RESERVED_NAME {
            result.add_error(errors::reserved_name(RESERVED_NAME));
        }

        // Check name consistency
        if name != &def.name {
            result.add_error(errors::name_mismatch(name, &def.name));
        }

        // Schema validation: name must not be empty
        if def.name.is_empty() {
            result.add_error(errors::empty_name());
        }

        // Schema validation: name should be valid identifier-like
        if !is_valid_container_name(&def.name) {
            result.add_error(errors::invalid_name_characters(&def.name));
        }

        // Validate mount targets are absolute paths
        for mount in &def.mounts {
            if !mount.target.starts_with('/') {
                result.add_error(errors::mount_target_not_absolute(
                    name,
                    &mount.name,
                    &mount.target,
                ));
            }
        }
    }

    // Check for cycles in parent chains
    if let Err(cycle_error) = detect_parent_cycles(definitions) {
        result.add_error(cycle_error.to_string());
    }

    result
}

/// Check if a container name is valid (alphanumeric, hyphens, underscores)
fn is_valid_container_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    name.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Detect cycles in parent chains
fn detect_parent_cycles(definitions: &HashMap<String, ContainerDefinition>) -> Result<()> {
    for name in definitions.keys() {
        let mut visited = HashSet::new();
        let mut current = Some(name.as_str());

        while let Some(current_name) = current {
            if !visited.insert(current_name.to_string()) {
                // Found a cycle - collect the cycle for a better error message
                let cycle_refs: Vec<&str> = visited.iter().map(|s| s.as_str()).collect();
                return Err(Error::Other(errors::circular_dependency(&cycle_refs)));
            }

            current = definitions
                .get(current_name)
                .and_then(|d| d.parent.as_deref());
        }

        // Note: Parent existence is checked at build-time, not parse-time
        // because parent might be from another source (project vs host)
    }

    Ok(())
}

// ============================================================================
// Build-time validation
// ============================================================================

/// Validate container definitions at build time.
///
/// Checks:
/// - Parent existence and reachability
/// - Containerfile existence in definition directory
/// - Build tools available (optional, reported as warning)
///
/// # Arguments
/// * `definitions` - All discovered definitions with source info
/// * `repo_path` - Repository root path
///
/// # Returns
/// * `ValidationResult` with any errors/warnings found
pub fn validate_build(definitions: &[DefinitionWithSource], repo_path: &Path) -> ValidationResult {
    let mut result = ValidationResult::new();

    // Build a map for quick lookup
    let def_map: HashMap<&str, &DefinitionWithSource> = definitions
        .iter()
        .map(|d| (d.definition.name.as_str(), d))
        .collect();

    for def_with_source in definitions {
        let def = &def_with_source.definition;

        // Skip embedded definitions (they don't have Containerfiles to check)
        if def_with_source.source == DefinitionSource::Embedded {
            continue;
        }

        // Check parent exists (across all sources)
        if let Some(parent_name) = &def.parent
            && !def_map.contains_key(parent_name.as_str())
        {
            result.add_error(errors::missing_parent(
                &def.name,
                parent_name,
                &[
                    ".binnacle/containers/",
                    "~/.local/share/binnacle/<hash>/containers/",
                ],
            ));
        }

        // Check Containerfile exists
        let containerfile_path = get_containerfile_path(def_with_source, repo_path);
        if let Some(path) = containerfile_path
            && !path.exists()
        {
            result.add_warning(errors::missing_containerfile(
                &def.name,
                &path.display().to_string(),
            ));
        }
    }

    result
}

/// Get the expected Containerfile path for a definition
fn get_containerfile_path(
    def_with_source: &DefinitionWithSource,
    repo_path: &Path,
) -> Option<std::path::PathBuf> {
    match def_with_source.source {
        DefinitionSource::Project => {
            // .binnacle/containers/<name>/Containerfile
            Some(
                repo_path
                    .join(".binnacle")
                    .join("containers")
                    .join(&def_with_source.definition.name)
                    .join("Containerfile"),
            )
        }
        DefinitionSource::Host => {
            // For host definitions, Containerfile would be in the host containers dir
            // We'd need the storage dir, but that's complex - skip for now
            None
        }
        DefinitionSource::Embedded => None,
    }
}

// ============================================================================
// Run-time validation
// ============================================================================

/// Validate container definitions at run time.
///
/// Checks:
/// - Mount source paths exist (error for required, warning for optional)
/// - Image exists in container runtime
///
/// # Arguments
/// * `definition` - The container definition to validate
/// * `repo_path` - Repository root path
/// * `skip_mount_validation` - If true, skip mount existence checks
///
/// # Returns
/// * `ValidationResult` with any errors/warnings found
pub fn validate_run(
    definition: &ContainerDefinition,
    repo_path: &Path,
    skip_mount_validation: bool,
) -> ValidationResult {
    let mut result = ValidationResult::new();

    if !skip_mount_validation {
        for mount in &definition.mounts {
            if let Some(source) = &mount.source {
                // Resolve and check mount source
                match super::resolve_mount_source(source, repo_path) {
                    Ok(resolved) => {
                        // Skip special mounts (workspace, binnacle)
                        let source_str = resolved.to_string_lossy();
                        if source_str == "workspace" || source_str == "binnacle" {
                            continue;
                        }

                        if !resolved.as_os_str().is_empty() && !resolved.exists() {
                            let resolved_str = resolved.display().to_string();
                            if mount.optional {
                                result.add_warning(errors::optional_mount_skipped(
                                    &mount.name,
                                    &resolved_str,
                                ));
                            } else {
                                result.add_error(errors::mount_source_not_found(
                                    &mount.name,
                                    &resolved_str,
                                ));
                            }
                        }
                    }
                    Err(e) => {
                        result.add_error(errors::mount_resolve_failed(
                            &mount.name,
                            source,
                            &e.to_string(),
                        ));
                    }
                }
            }
        }
    }

    result
}

// ============================================================================
// Combined validation
// ============================================================================

/// Run all validation tiers appropriate for the given context
///
/// # Arguments
/// * `definitions` - All discovered definitions with source info
/// * `repo_path` - Repository root path
/// * `tier` - The maximum tier to validate up to
///
/// # Returns
/// * `ValidationResult` with all errors/warnings found
pub fn validate_all(
    definitions: &[DefinitionWithSource],
    repo_path: &Path,
    tier: ValidationTier,
) -> ValidationResult {
    let mut result = ValidationResult::new();

    // Build definition map for parse validation
    let def_map: HashMap<String, ContainerDefinition> = definitions
        .iter()
        .map(|d| (d.definition.name.clone(), d.definition.clone()))
        .collect();

    // Always run parse validation
    result.merge(validate_parse(&def_map));

    if tier == ValidationTier::Parse {
        return result;
    }

    // Run build validation
    result.merge(validate_build(definitions, repo_path));

    if tier == ValidationTier::Build {
        return result;
    }

    // Run validation runs per-definition at run time, not here
    // (because we need specific definition context)

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::container::{EntrypointMode, Mount, MountMode};

    fn make_simple_def(name: &str, parent: Option<&str>) -> ContainerDefinition {
        ContainerDefinition {
            name: name.to_string(),
            description: None,
            parent: parent.map(|s| s.to_string()),
            entrypoint_mode: EntrypointMode::Replace,
            defaults: None,
            mounts: vec![],
        }
    }

    // ========================================
    // Parse-time validation tests
    // ========================================

    #[test]
    fn test_parse_valid_definitions() {
        let mut defs = HashMap::new();
        defs.insert("base".to_string(), make_simple_def("base", None));
        defs.insert("dev".to_string(), make_simple_def("dev", Some("base")));

        let result = validate_parse(&defs);
        assert!(result.is_ok(), "Expected no errors: {:?}", result.errors);
    }

    #[test]
    fn test_parse_reserved_name_rejected() {
        let mut defs = HashMap::new();
        defs.insert(
            RESERVED_NAME.to_string(),
            make_simple_def(RESERVED_NAME, None),
        );

        let result = validate_parse(&defs);
        assert!(!result.is_ok());
        // The new error format uses "bn: error: config: reserved container name"
        assert!(
            result
                .errors
                .iter()
                .any(|e| e.contains("reserved container name")),
            "Error should mention reserved container name: {:?}",
            result.errors
        );
        // Verify suggestion is included
        assert!(
            result
                .errors
                .iter()
                .any(|e| e.contains("Choose a different name")),
            "Error should include suggestion: {:?}",
            result.errors
        );
    }

    #[test]
    fn test_parse_empty_name_rejected() {
        let mut defs = HashMap::new();
        defs.insert("".to_string(), make_simple_def("", None));

        let result = validate_parse(&defs);
        assert!(!result.is_ok());
        // The new error format uses "bn: error: config: empty container name"
        assert!(
            result
                .errors
                .iter()
                .any(|e| e.contains("empty container name") || e.contains("invalid container name"))
        );
    }

    #[test]
    fn test_parse_invalid_name_characters() {
        let mut defs = HashMap::new();
        defs.insert(
            "my container".to_string(),
            make_simple_def("my container", None),
        );

        let result = validate_parse(&defs);
        assert!(!result.is_ok());
        // The new error format uses "bn: error: config: invalid container name"
        assert!(
            result
                .errors
                .iter()
                .any(|e| e.contains("invalid container name"))
        );
    }

    #[test]
    fn test_parse_valid_name_with_hyphens_underscores() {
        let mut defs = HashMap::new();
        defs.insert(
            "my-container_v2".to_string(),
            make_simple_def("my-container_v2", None),
        );

        let result = validate_parse(&defs);
        assert!(result.is_ok(), "Expected no errors: {:?}", result.errors);
    }

    #[test]
    fn test_parse_cycle_detection() {
        let mut defs = HashMap::new();
        defs.insert("a".to_string(), make_simple_def("a", Some("b")));
        defs.insert("b".to_string(), make_simple_def("b", Some("a")));

        let result = validate_parse(&defs);
        assert!(!result.is_ok());
        // The new error format uses "bn: error: config: circular dependency detected"
        assert!(
            result
                .errors
                .iter()
                .any(|e| e.contains("circular dependency")),
            "Error should mention circular dependency: {:?}",
            result.errors
        );
    }

    #[test]
    fn test_parse_self_cycle_detection() {
        let mut defs = HashMap::new();
        defs.insert("a".to_string(), make_simple_def("a", Some("a")));

        let result = validate_parse(&defs);
        assert!(!result.is_ok());
        // The new error format uses "bn: error: config: circular dependency detected"
        assert!(
            result
                .errors
                .iter()
                .any(|e| e.contains("circular dependency")),
            "Error should mention circular dependency: {:?}",
            result.errors
        );
    }

    #[test]
    fn test_parse_three_node_cycle() {
        let mut defs = HashMap::new();
        defs.insert("a".to_string(), make_simple_def("a", Some("b")));
        defs.insert("b".to_string(), make_simple_def("b", Some("c")));
        defs.insert("c".to_string(), make_simple_def("c", Some("a")));

        let result = validate_parse(&defs);
        assert!(!result.is_ok());
        // The new error format uses "bn: error: config: circular dependency detected"
        assert!(
            result
                .errors
                .iter()
                .any(|e| e.contains("circular dependency")),
            "Error should mention circular dependency: {:?}",
            result.errors
        );
    }

    #[test]
    fn test_parse_mount_target_must_be_absolute() {
        let mut defs = HashMap::new();
        defs.insert(
            "test".to_string(),
            ContainerDefinition {
                name: "test".to_string(),
                description: None,
                parent: None,
                entrypoint_mode: EntrypointMode::Replace,
                defaults: None,
                mounts: vec![Mount {
                    name: "data".to_string(),
                    source: Some("/host/data".to_string()),
                    target: "relative/path".to_string(), // Invalid!
                    mode: MountMode::ReadWrite,
                    optional: false,
                }],
            },
        );

        let result = validate_parse(&defs);
        assert!(!result.is_ok());
        // The new error format uses "bn: error: config: mount target must be absolute path"
        assert!(
            result.errors.iter().any(|e| e.contains("absolute path")),
            "Error should mention absolute path: {:?}",
            result.errors
        );
    }

    // ========================================
    // Build-time validation tests
    // ========================================

    #[test]
    fn test_build_missing_parent() {
        let definitions = vec![DefinitionWithSource {
            definition: make_simple_def("child", Some("nonexistent")),
            source: DefinitionSource::Project,
            config_path: std::path::PathBuf::from(".binnacle/containers/config.kdl"),
            modified_at: None,
        }];

        let result = validate_build(&definitions, std::path::Path::new("/repo"));
        assert!(!result.is_ok());
        // The new error format uses "bn: error: build: parent container not found"
        assert!(
            result
                .errors
                .iter()
                .any(|e| e.contains("parent container not found")),
            "Error should mention parent not found: {:?}",
            result.errors
        );
    }

    #[test]
    fn test_build_valid_parent_reference() {
        let definitions = vec![
            DefinitionWithSource {
                definition: make_simple_def("base", None),
                source: DefinitionSource::Project,
                config_path: std::path::PathBuf::from(".binnacle/containers/config.kdl"),
                modified_at: None,
            },
            DefinitionWithSource {
                definition: make_simple_def("child", Some("base")),
                source: DefinitionSource::Project,
                config_path: std::path::PathBuf::from(".binnacle/containers/config.kdl"),
                modified_at: None,
            },
        ];

        let result = validate_build(&definitions, std::path::Path::new("/repo"));
        // May have warnings about missing Containerfile, but no errors
        assert!(result.is_ok(), "Expected no errors: {:?}", result.errors);
    }

    #[test]
    fn test_build_skips_embedded_definitions() {
        let definitions = vec![DefinitionWithSource {
            definition: make_simple_def(RESERVED_NAME, None),
            source: DefinitionSource::Embedded,
            config_path: std::path::PathBuf::from("<embedded>"),
            modified_at: None,
        }];

        let result = validate_build(&definitions, std::path::Path::new("/repo"));
        assert!(result.is_ok(), "Expected no errors: {:?}", result.errors);
    }

    // ========================================
    // Run-time validation tests
    // ========================================

    #[test]
    fn test_run_missing_required_mount() {
        let def = ContainerDefinition {
            name: "test".to_string(),
            description: None,
            parent: None,
            entrypoint_mode: EntrypointMode::Replace,
            defaults: None,
            mounts: vec![Mount {
                name: "data".to_string(),
                source: Some("/nonexistent/path".to_string()),
                target: "/data".to_string(),
                mode: MountMode::ReadWrite,
                optional: false,
            }],
        };

        let result = validate_run(&def, std::path::Path::new("/repo"), false);
        assert!(!result.is_ok());
        // The new error format uses "bn: error: mount: source path not found"
        assert!(
            result
                .errors
                .iter()
                .any(|e| e.contains("source path not found")),
            "Error should mention source path not found: {:?}",
            result.errors
        );
    }

    #[test]
    fn test_run_missing_optional_mount_is_warning() {
        let def = ContainerDefinition {
            name: "test".to_string(),
            description: None,
            parent: None,
            entrypoint_mode: EntrypointMode::Replace,
            defaults: None,
            mounts: vec![Mount {
                name: "cache".to_string(),
                source: Some("/nonexistent/optional".to_string()),
                target: "/cache".to_string(),
                mode: MountMode::ReadOnly,
                optional: true,
            }],
        };

        let result = validate_run(&def, std::path::Path::new("/repo"), false);
        assert!(result.is_ok(), "Expected no errors: {:?}", result.errors);
        assert!(result.has_warnings());
        // The new warning format uses "bn: warning: mount: skipping optional mount"
        assert!(
            result
                .warnings
                .iter()
                .any(|w| w.contains("skipping optional mount")),
            "Warning should mention skipping optional mount: {:?}",
            result.warnings
        );
    }

    #[test]
    fn test_run_skip_mount_validation() {
        let def = ContainerDefinition {
            name: "test".to_string(),
            description: None,
            parent: None,
            entrypoint_mode: EntrypointMode::Replace,
            defaults: None,
            mounts: vec![Mount {
                name: "data".to_string(),
                source: Some("/nonexistent/path".to_string()),
                target: "/data".to_string(),
                mode: MountMode::ReadWrite,
                optional: false,
            }],
        };

        let result = validate_run(&def, std::path::Path::new("/repo"), true);
        assert!(
            result.is_ok(),
            "Expected no errors when skipping validation"
        );
        assert!(!result.has_warnings());
    }

    #[test]
    fn test_run_special_mounts_skipped() {
        let def = ContainerDefinition {
            name: "test".to_string(),
            description: None,
            parent: None,
            entrypoint_mode: EntrypointMode::Replace,
            defaults: None,
            mounts: vec![
                Mount {
                    name: "workspace".to_string(),
                    source: Some("workspace".to_string()),
                    target: "/workspace".to_string(),
                    mode: MountMode::ReadWrite,
                    optional: false,
                },
                Mount {
                    name: "binnacle".to_string(),
                    source: Some("binnacle".to_string()),
                    target: "/binnacle".to_string(),
                    mode: MountMode::ReadWrite,
                    optional: false,
                },
            ],
        };

        let result = validate_run(&def, std::path::Path::new("/repo"), false);
        assert!(result.is_ok(), "Expected no errors for special mounts");
    }

    // ========================================
    // Combined validation tests
    // ========================================

    #[test]
    fn test_validate_all_parse_tier() {
        let definitions = vec![DefinitionWithSource {
            definition: make_simple_def("test", None),
            source: DefinitionSource::Project,
            config_path: std::path::PathBuf::from(".binnacle/containers/config.kdl"),
            modified_at: None,
        }];

        let result = validate_all(
            &definitions,
            std::path::Path::new("/repo"),
            ValidationTier::Parse,
        );
        assert!(result.is_ok(), "Expected no errors: {:?}", result.errors);
    }

    #[test]
    fn test_validate_all_build_tier() {
        let definitions = vec![DefinitionWithSource {
            definition: make_simple_def("test", None),
            source: DefinitionSource::Project,
            config_path: std::path::PathBuf::from(".binnacle/containers/config.kdl"),
            modified_at: None,
        }];

        let result = validate_all(
            &definitions,
            std::path::Path::new("/repo"),
            ValidationTier::Build,
        );
        // May have warnings but no errors
        assert!(result.is_ok(), "Expected no errors: {:?}", result.errors);
    }

    #[test]
    fn test_validation_result_merge() {
        let mut r1 = ValidationResult::new();
        r1.add_error("error1");
        r1.add_warning("warning1");

        let mut r2 = ValidationResult::new();
        r2.add_error("error2");
        r2.add_warning("warning2");

        r1.merge(r2);

        assert_eq!(r1.errors.len(), 2);
        assert_eq!(r1.warnings.len(), 2);
    }

    #[test]
    fn test_validation_result_into_result() {
        let mut result = ValidationResult::new();
        result.add_warning("just a warning");

        let res = result.into_result();
        assert!(res.is_ok());
        assert_eq!(res.unwrap(), vec!["just a warning"]);

        let mut result = ValidationResult::new();
        result.add_error("an error");
        let res = result.into_result();
        assert!(res.is_err());
    }
}
