//! Container definition management and config.kdl parsing.

pub mod errors;
pub mod validation;

use crate::storage::get_storage_dir;
use crate::{Error, Result};
use kdl::{KdlDocument, KdlNode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Reserved container name that cannot be used by projects/users.
pub const RESERVED_NAME: &str = "binnacle";

/// Name for the embedded default container (minimal base image).
pub const EMBEDDED_DEFAULT_NAME: &str = "default";

/// Container definition parsed from config.kdl
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContainerDefinition {
    pub name: String,
    pub description: Option<String>,
    pub parent: Option<String>,
    pub defaults: Option<Defaults>,
    pub mounts: Vec<Mount>,
}

/// Default resource limits for a container
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Defaults {
    pub cpus: Option<u32>,
    pub memory: Option<String>,
}

/// Container mount configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Mount {
    pub name: String,
    pub source: Option<String>,
    pub target: String,
    pub mode: MountMode,
    pub optional: bool,
}

/// Mount mode (read-only or read-write)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MountMode {
    #[serde(rename = "ro")]
    ReadOnly,
    #[serde(rename = "rw")]
    ReadWrite,
}

impl std::str::FromStr for MountMode {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "ro" => Ok(Self::ReadOnly),
            "rw" => Ok(Self::ReadWrite),
            _ => Err(Error::Other(errors::invalid_mount_mode(s))),
        }
    }
}

/// Resolve mount source path to an absolute path.
///
/// # Resolution Rules
/// - Special values ("workspace", "binnacle"): Return as-is (handled at runtime)
/// - Absolute paths (starting with '/'): Return as-is
/// - Home expansion ('~' or '$HOME'): Expand to user's home directory
/// - Relative paths: Resolve from repo_root
///
/// # Arguments
/// - `source`: The mount source path from config
/// - `repo_root`: The repository root directory (for relative path resolution)
///
/// # Returns
/// - Resolved absolute path, or original value for special mount names
pub fn resolve_mount_source(source: &str, repo_root: &Path) -> Result<PathBuf> {
    // Special mount names - return as-is
    if source == "workspace" || source == "binnacle" {
        return Ok(PathBuf::from(source));
    }

    // Absolute paths - return as-is
    if source.starts_with('/') {
        return Ok(PathBuf::from(source));
    }

    // Home expansion - handle both ~ and $HOME
    if let Some(expanded) = expand_home(source)? {
        return Ok(expanded);
    }

    // Relative paths - resolve from repo root
    let resolved = repo_root.join(source);
    Ok(resolved)
}

/// Expand home directory in path.
/// Returns Some(path) if expansion occurred, None otherwise.
fn expand_home(path: &str) -> Result<Option<PathBuf>> {
    if path.starts_with("~/") || path == "~" {
        let home =
            dirs::home_dir().ok_or_else(|| Error::Other(errors::home_expansion_failed("~")))?;

        if path == "~" {
            return Ok(Some(home));
        }

        // Replace ~ with home directory
        let rest = &path[2..]; // Skip "~/"
        return Ok(Some(home.join(rest)));
    }

    if path.starts_with("$HOME/") || path == "$HOME" {
        let home =
            dirs::home_dir().ok_or_else(|| Error::Other(errors::home_expansion_failed("$HOME")))?;

        if path == "$HOME" {
            return Ok(Some(home));
        }

        // Replace $HOME with home directory
        let rest = &path[6..]; // Skip "$HOME/"
        return Ok(Some(home.join(rest)));
    }

    Ok(None)
}

/// Result of mount validation
#[derive(Debug, Clone)]
pub struct ValidatedMount {
    pub mount: Mount,
    pub resolved_source: PathBuf,
}

/// Validate mounts and resolve their source paths.
///
/// # Behavior
/// - Required mounts (optional=false): Error if source path doesn't exist
/// - Optional mounts (optional=true): Skip with warning if source path doesn't exist
///
/// # Arguments
/// - `mounts`: The list of mounts from container definition
/// - `repo_root`: The repository root directory (for relative path resolution)
///
/// # Returns
/// - List of validated mounts with resolved source paths
/// - Skips optional mounts that don't exist (logs warning)
/// - Errors on missing required mounts
///
/// # Example
/// ```
/// use binnacle::container::{Mount, MountMode, validate_mounts};
/// use std::path::Path;
///
/// let mounts = vec![
///     Mount {
///         name: "workspace".to_string(),
///         source: Some("workspace".to_string()),
///         target: "/workspace".to_string(),
///         mode: MountMode::ReadWrite,
///         optional: false,
///     },
///     Mount {
///         name: "cache".to_string(),
///         source: Some("~/.cargo".to_string()),
///         target: "/cargo-cache".to_string(),
///         mode: MountMode::ReadOnly,
///         optional: true,
///     },
/// ];
///
/// let repo_root = Path::new("/repo");
/// let validated = validate_mounts(&mounts, repo_root).unwrap();
/// // Optional cache mount will be skipped with warning if ~/.cargo doesn't exist
/// // Required workspace mount will error if it doesn't exist
/// ```
pub fn validate_mounts(mounts: &[Mount], repo_root: &Path) -> Result<Vec<ValidatedMount>> {
    let mut validated = Vec::new();

    for mount in mounts {
        // Resolve the source path (if source is specified)
        let resolved_source = if let Some(source) = &mount.source {
            resolve_mount_source(source, repo_root)?
        } else {
            // No source means special mount (like workspace/binnacle) - handled at runtime
            PathBuf::from("")
        };

        // Check if the source path exists (skip special mounts like "workspace"/"binnacle")
        let is_special_mount = resolved_source.to_string_lossy() == "workspace"
            || resolved_source.to_string_lossy() == "binnacle";

        if !is_special_mount && !resolved_source.as_os_str().is_empty() && !resolved_source.exists()
        {
            if mount.optional {
                // Optional mount missing - log warning and skip
                eprintln!(
                    "{}",
                    errors::optional_mount_skipped(
                        &mount.name,
                        &resolved_source.display().to_string()
                    )
                );
                continue;
            } else {
                // Required mount missing - error
                return Err(Error::Other(errors::mount_source_not_found(
                    &mount.name,
                    &resolved_source.display().to_string(),
                )));
            }
        }

        validated.push(ValidatedMount {
            mount: mount.clone(),
            resolved_source,
        });
    }

    Ok(validated)
}

/// Parse config.kdl document into a map of container definitions
pub fn parse_config_kdl(doc: &KdlDocument) -> Result<HashMap<String, ContainerDefinition>> {
    let mut definitions = HashMap::new();

    for node in doc.nodes() {
        if node.name().to_string() != "container" {
            continue;
        }

        let def = parse_container_node(node)?;

        // Validate reserved name
        if def.name == RESERVED_NAME {
            return Err(Error::Other(errors::reserved_name(RESERVED_NAME)));
        }

        definitions.insert(def.name.clone(), def);
    }

    Ok(definitions)
}

/// Parse a single container node
fn parse_container_node(node: &KdlNode) -> Result<ContainerDefinition> {
    // Get container name from first argument
    let name = node
        .entries()
        .first()
        .and_then(|e| e.value().as_string())
        .ok_or_else(|| Error::Other(errors::missing_container_name()))?
        .to_string();

    let mut description = None;
    let mut parent = None;
    let mut defaults = None;
    let mut mounts = Vec::new();

    // Parse children nodes
    if let Some(children) = node.children() {
        for child in children.nodes() {
            match child.name().to_string().as_str() {
                "description" => {
                    description = child
                        .entries()
                        .first()
                        .and_then(|e| e.value().as_string())
                        .map(|s| s.to_string());
                }
                "parent" => {
                    parent = child
                        .entries()
                        .first()
                        .and_then(|e| e.value().as_string())
                        .map(|s| s.to_string());
                }
                "defaults" => {
                    defaults = Some(parse_defaults_node(child)?);
                }
                "mounts" => {
                    mounts = parse_mounts_node(child)?;
                }
                _ => {}
            }
        }
    }

    Ok(ContainerDefinition {
        name,
        description,
        parent,
        defaults,
        mounts,
    })
}

/// Parse defaults node
fn parse_defaults_node(node: &KdlNode) -> Result<Defaults> {
    let mut cpus = None;
    let mut memory = None;

    if let Some(children) = node.children() {
        for child in children.nodes() {
            match child.name().to_string().as_str() {
                "cpus" => {
                    cpus = child
                        .entries()
                        .first()
                        .and_then(|e| e.value().as_integer())
                        .map(|v| v as u32);
                }
                "memory" => {
                    memory = child
                        .entries()
                        .first()
                        .and_then(|e| e.value().as_string())
                        .map(|s| s.to_string());
                }
                _ => {}
            }
        }
    }

    Ok(Defaults { cpus, memory })
}

/// Parse mounts node
fn parse_mounts_node(node: &KdlNode) -> Result<Vec<Mount>> {
    let mut mounts = Vec::new();

    if let Some(children) = node.children() {
        for child in children.nodes() {
            if child.name().to_string() == "mount" {
                mounts.push(parse_mount_node(child)?);
            }
        }
    }

    Ok(mounts)
}

/// Parse a single mount node
fn parse_mount_node(node: &KdlNode) -> Result<Mount> {
    // Get mount name from first argument
    let name = node
        .entries()
        .first()
        .and_then(|e| e.value().as_string())
        .ok_or_else(|| Error::Other(errors::missing_mount_name()))?
        .to_string();

    // Get target (required)
    let target = node
        .get("target")
        .and_then(|v| v.as_string())
        .ok_or_else(|| Error::Other(errors::missing_mount_target(&name)))?
        .to_string();

    // Get source (optional for special mounts like workspace/binnacle)
    let source = node
        .get("source")
        .and_then(|v| v.as_string())
        .map(|s| s.to_string());

    // Get mode (default to rw)
    let mode = node
        .get("mode")
        .and_then(|v| v.as_string())
        .map(|s| s.parse())
        .transpose()?
        .unwrap_or(MountMode::ReadWrite);

    // Get optional flag (default to false)
    let optional = node
        .get("optional")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    Ok(Mount {
        name,
        source,
        target,
        mode,
        optional,
    })
}

/// Source of a container definition
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DefinitionSource {
    /// Project-level definition (.binnacle/containers/)
    Project,
    /// User-level definition (~/.local/share/binnacle/<hash>/containers/)
    Host,
    /// Embedded fallback (compiled-in)
    Embedded,
}

impl std::fmt::Display for DefinitionSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Project => write!(f, "project"),
            Self::Host => write!(f, "host"),
            Self::Embedded => write!(f, "embedded"),
        }
    }
}

/// Container definition with source metadata
#[derive(Debug, Clone)]
pub struct DefinitionWithSource {
    pub definition: ContainerDefinition,
    pub source: DefinitionSource,
    pub config_path: PathBuf,
    pub modified_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Conflict information when same name exists in multiple sources
#[derive(Debug, Clone)]
pub struct DefinitionConflict {
    pub name: String,
    pub project_def: DefinitionWithSource,
    pub host_def: DefinitionWithSource,
}

impl std::fmt::Display for DefinitionConflict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "Conflict: definition '{}' exists in multiple sources:",
            self.name
        )?;
        writeln!(f)?;
        writeln!(f, "  PROJECT (.binnacle/containers/):")?;
        if let Some(desc) = &self.project_def.definition.description {
            writeln!(f, "    Description: {}", desc)?;
        }
        if let Some(modified) = &self.project_def.modified_at {
            writeln!(
                f,
                "    Modified: {}",
                modified.format("%Y-%m-%d %H:%M:%S UTC")
            )?;
        }
        writeln!(f, "    Path: {}", self.project_def.config_path.display())?;
        writeln!(f)?;
        writeln!(f, "  HOST (~/.local/share/binnacle/<hash>/containers/):")?;
        if let Some(desc) = &self.host_def.definition.description {
            writeln!(f, "    Description: {}", desc)?;
        }
        if let Some(modified) = &self.host_def.modified_at {
            writeln!(
                f,
                "    Modified: {}",
                modified.format("%Y-%m-%d %H:%M:%S UTC")
            )?;
        }
        writeln!(f, "    Path: {}", self.host_def.config_path.display())?;
        writeln!(f)?;
        write!(
            f,
            "Use --project or --host to specify which definition to use."
        )
    }
}

/// Source preference for resolving definition conflicts
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourcePreference {
    /// No preference - error on conflict
    None,
    /// Prefer project-level definition
    Project,
    /// Prefer host-level definition
    Host,
}

/// Detect conflicts in discovered definitions
///
/// Returns a list of conflicts where the same name exists in both project and host sources.
pub fn detect_conflicts(definitions: &[DefinitionWithSource]) -> Vec<DefinitionConflict> {
    use std::collections::HashMap;

    // Group definitions by name
    let mut by_name: HashMap<&str, Vec<&DefinitionWithSource>> = HashMap::new();
    for def in definitions {
        by_name.entry(&def.definition.name).or_default().push(def);
    }

    // Find conflicts (name exists in both project and host)
    let mut conflicts = Vec::new();
    for (name, defs) in by_name {
        let project_def = defs.iter().find(|d| d.source == DefinitionSource::Project);
        let host_def = defs.iter().find(|d| d.source == DefinitionSource::Host);

        if let (Some(project), Some(host)) = (project_def, host_def) {
            conflicts.push(DefinitionConflict {
                name: name.to_string(),
                project_def: (*project).clone(),
                host_def: (*host).clone(),
            });
        }
    }

    conflicts
}

/// Resolve a specific definition by name, respecting source preference
///
/// # Arguments
/// - `definitions`: All discovered definitions
/// - `name`: Name of the definition to find
/// - `preference`: Source preference for conflict resolution
///
/// # Returns
/// - `Ok(DefinitionWithSource)`: The resolved definition
/// - `Err(_)`: If not found, or if conflict exists with no preference
pub fn resolve_definition(
    definitions: &[DefinitionWithSource],
    name: &str,
    preference: SourcePreference,
) -> Result<DefinitionWithSource> {
    // Find all definitions with this name
    let matches: Vec<_> = definitions
        .iter()
        .filter(|d| d.definition.name == name)
        .collect();

    if matches.is_empty() {
        return Err(Error::Other(errors::definition_not_found(
            name,
            &[
                ".binnacle/containers/config.kdl",
                "~/.local/share/binnacle/<hash>/containers/config.kdl",
            ],
        )));
    }

    // If only one match, return it
    if matches.len() == 1 {
        return Ok(matches[0].clone());
    }

    // Multiple matches - check for conflict and apply preference
    let project_def = matches
        .iter()
        .find(|d| d.source == DefinitionSource::Project);
    let host_def = matches.iter().find(|d| d.source == DefinitionSource::Host);

    match (project_def, host_def, preference) {
        (Some(project), Some(_host), SourcePreference::Project) => Ok((*project).clone()),
        (Some(_project), Some(host), SourcePreference::Host) => Ok((*host).clone()),
        (Some(project), Some(host), SourcePreference::None) => {
            // Conflict with no preference - return error
            let conflict = DefinitionConflict {
                name: name.to_string(),
                project_def: (*project).clone(),
                host_def: (*host).clone(),
            };
            Err(Error::Other(conflict.to_string()))
        }
        // One of them is Some, pick that one
        (Some(def), None, _) => Ok((*def).clone()),
        (None, Some(def), _) => Ok((*def).clone()),
        _ => {
            // Fall back to first match (shouldn't happen with project/host sources)
            Ok(matches[0].clone())
        }
    }
}

/// Discover container definitions from all sources
///
/// Search order:
/// 1. Project-level: .binnacle/containers/config.kdl
/// 2. User-level: ~/.local/share/binnacle/<hash>/containers/config.kdl
/// 3. Embedded fallback: only the reserved "binnacle" definition
///
/// Returns a map of container name -> definition with source metadata.
/// If the same name exists in multiple sources, both are included (caller must resolve conflicts).
pub fn discover_definitions(repo_path: &Path) -> Result<Vec<DefinitionWithSource>> {
    let mut results = Vec::new();

    // Helper to get file modification time
    fn get_modified_time(path: &Path) -> Option<chrono::DateTime<chrono::Utc>> {
        use chrono::{DateTime, Utc};
        path.metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .map(DateTime::<Utc>::from)
    }

    // 1. Check project-level: .binnacle/containers/config.kdl
    let project_config = repo_path
        .join(".binnacle")
        .join("containers")
        .join("config.kdl");
    if project_config.exists() {
        let content = fs::read_to_string(&project_config).map_err(|e| {
            Error::Other(errors::config_read_failed(
                &project_config.display().to_string(),
                &e.to_string(),
            ))
        })?;
        let doc: KdlDocument = content.parse().map_err(|e: kdl::KdlError| {
            Error::Other(errors::config_parse_failed(
                &project_config.display().to_string(),
                &e.to_string(),
            ))
        })?;
        let defs = parse_config_kdl(&doc)?;
        let modified_at = get_modified_time(&project_config);

        for (_, def) in defs {
            results.push(DefinitionWithSource {
                definition: def,
                source: DefinitionSource::Project,
                config_path: project_config.clone(),
                modified_at,
            });
        }
    }

    // 2. Check user-level: ~/.local/share/binnacle/<hash>/containers/config.kdl
    let storage_dir = get_storage_dir(repo_path)?;
    let host_config = storage_dir.join("containers").join("config.kdl");
    if host_config.exists() {
        let content = fs::read_to_string(&host_config).map_err(|e| {
            Error::Other(errors::config_read_failed(
                &host_config.display().to_string(),
                &e.to_string(),
            ))
        })?;
        let doc: KdlDocument = content.parse().map_err(|e: kdl::KdlError| {
            Error::Other(errors::config_parse_failed(
                &host_config.display().to_string(),
                &e.to_string(),
            ))
        })?;
        let defs = parse_config_kdl(&doc)?;
        let modified_at = get_modified_time(&host_config);

        for (_, def) in defs {
            results.push(DefinitionWithSource {
                definition: def,
                source: DefinitionSource::Host,
                config_path: host_config.clone(),
                modified_at,
            });
        }
    }

    // 3. Always include embedded "binnacle" and "default" definitions
    // These are foundational layers that custom containers can depend on via parent inheritance
    results.push(DefinitionWithSource {
        definition: ContainerDefinition {
            name: RESERVED_NAME.to_string(),
            description: Some(
                "Embedded binnacle-worker container (full development environment)".to_string(),
            ),
            parent: Some(EMBEDDED_DEFAULT_NAME.to_string()),
            defaults: None,
            mounts: vec![],
        },
        source: DefinitionSource::Embedded,
        config_path: PathBuf::from("<embedded>"),
        modified_at: None,
    });

    // Add embedded default container (minimal base image)
    results.push(DefinitionWithSource {
        definition: ContainerDefinition {
            name: EMBEDDED_DEFAULT_NAME.to_string(),
            description: Some(
                "Embedded binnacle-default container (minimal base with bn + copilot)".to_string(),
            ),
            parent: None,
            defaults: None,
            mounts: vec![],
        },
        source: DefinitionSource::Embedded,
        config_path: PathBuf::from("<embedded>"),
        modified_at: None,
    });

    Ok(results)
}

/// Compute repository hash (first 12 chars of SHA256 of canonical path)
pub fn compute_repo_hash(repo_path: &Path) -> Result<String> {
    use sha2::{Digest, Sha256};

    let canonical = repo_path
        .canonicalize()
        .map_err(|e| Error::Other(format!("Failed to canonicalize repo path: {}", e)))?;

    let mut hasher = Sha256::new();
    hasher.update(canonical.to_string_lossy().as_bytes());
    let hash = hasher.finalize();

    Ok(format!("{:x}", hash)[..12].to_string())
}

/// Generate Docker/Podman image name for a container definition
///
/// Format: `localhost/bn-<repo-hash>-<definition-name>:latest`
///
/// # Arguments
/// - `repo_path`: Repository root path (for hash computation)
/// - `definition_name`: Name of the container definition
///
/// # Example
/// ```no_run
/// use std::path::Path;
/// use binnacle::container::generate_image_name;
///
/// let image = generate_image_name(Path::new("/workspace"), "base").unwrap();
/// // Returns: "localhost/bn-a1b2c3d4e5f6-base:latest"
/// ```
pub fn generate_image_name(repo_path: &Path, definition_name: &str) -> Result<String> {
    let hash = compute_repo_hash(repo_path)?;
    Ok(format!("localhost/bn-{}-{}:latest", hash, definition_name))
}

/// Generate image name for a definition, handling embedded definitions specially.
///
/// Embedded definitions (binnacle, default) use hardcoded names:
/// - "binnacle" -> "localhost/binnacle-worker:latest"
/// - "default" -> "localhost/binnacle-default:latest"
///
/// Custom definitions use repository-scoped names.
///
/// # Arguments
/// - `repo_path`: Path to repository root
/// - `definition_name`: Name of the container definition
/// - `tag`: Tag to use (e.g., "latest")
/// - `is_embedded`: Whether this is an embedded definition
pub fn generate_image_name_for_definition(
    repo_path: &Path,
    definition_name: &str,
    tag: &str,
    is_embedded: bool,
) -> Result<String> {
    if is_embedded {
        // Embedded definitions use hardcoded names
        if definition_name == RESERVED_NAME {
            Ok(format!("localhost/binnacle-worker:{}", tag))
        } else if definition_name == EMBEDDED_DEFAULT_NAME {
            Ok(format!("localhost/binnacle-default:{}", tag))
        } else {
            Err(Error::Other(format!(
                "Unknown embedded container definition: {}",
                definition_name
            )))
        }
    } else {
        // Custom definitions use repository-scoped names
        let hash = compute_repo_hash(repo_path)?;
        Ok(format!("localhost/bn-{}-{}:{}", hash, definition_name, tag))
    }
}

/// Compute build order for container definitions (topological sort)
///
/// Returns definitions in dependency order (parents before children).
/// Detects circular dependencies and errors if found.
///
/// # Arguments
/// - `definitions`: Map of container name -> definition
///
/// # Returns
/// - `Ok(Vec<String>)`: Container names in build order
/// - `Err(_)`: If circular dependency detected or missing parent
///
/// # Example
/// Given definitions:
/// - base (no parent)
/// - rust-dev (parent: base)
/// - app (parent: rust-dev)
///
/// Returns: ["base", "rust-dev", "app"]
pub fn compute_build_order(
    definitions: &HashMap<String, ContainerDefinition>,
) -> Result<Vec<String>> {
    use std::collections::{HashSet, VecDeque};

    let mut in_degree: HashMap<String, usize> = HashMap::new();
    let mut children: HashMap<String, Vec<String>> = HashMap::new();
    let mut all_names: HashSet<String> = HashSet::new();

    // Initialize graph
    for (name, def) in definitions {
        all_names.insert(name.clone());
        in_degree.entry(name.clone()).or_insert(0);

        if let Some(parent) = &def.parent {
            // Validate parent exists
            if !definitions.contains_key(parent) {
                return Err(Error::Other(errors::missing_parent(
                    name,
                    parent,
                    &[
                        ".binnacle/containers/",
                        "~/.local/share/binnacle/<hash>/containers/",
                    ],
                )));
            }

            // Increment in-degree for this node (it has a dependency)
            *in_degree.entry(name.clone()).or_insert(0) += 1;

            // Add to parent's children list
            children
                .entry(parent.clone())
                .or_default()
                .push(name.clone());
        }
    }

    // Kahn's algorithm for topological sort
    let mut queue: VecDeque<String> = VecDeque::new();
    let mut result: Vec<String> = Vec::new();

    // Start with nodes that have no dependencies (in-degree = 0)
    for (name, &degree) in &in_degree {
        if degree == 0 {
            queue.push_back(name.clone());
        }
    }

    while let Some(current) = queue.pop_front() {
        result.push(current.clone());

        // Process children of current node
        if let Some(child_list) = children.get(&current) {
            for child in child_list {
                if let Some(degree) = in_degree.get_mut(child) {
                    *degree -= 1;
                    if *degree == 0 {
                        queue.push_back(child.clone());
                    }
                }
            }
        }
    }

    // If result doesn't contain all nodes, there's a cycle
    if result.len() != all_names.len() {
        // Find nodes in cycle for better error message
        let processed: HashSet<_> = result.iter().cloned().collect();
        let in_cycle: Vec<_> = all_names.difference(&processed).cloned().collect();
        let cycle_refs: Vec<&str> = in_cycle.iter().map(|s| s.as_str()).collect();

        return Err(Error::Other(errors::circular_dependency(&cycle_refs)));
    }

    Ok(result)
}

/// Determine the entrypoint path for a container definition.
///
/// # Arguments
/// * `definition` - Container definition
/// * `definitions` - All available definitions (for resolving parent)
///
/// # Returns
/// The entrypoint path that this container will use.
///
/// # Notes
/// Currently returns "/entrypoint.sh" as the standard path.
/// Future enhancement: parse Containerfile to extract actual ENTRYPOINT directive.
pub fn get_entrypoint_path(
    _definition: &ContainerDefinition,
    _definitions: &HashMap<String, ContainerDefinition>,
) -> String {
    // For now, we assume all containers use /entrypoint.sh as the standard path
    // Future enhancement: parse Containerfile ENTRYPOINT directive
    "/entrypoint.sh".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_container() {
        let kdl = r#"
container "base" {
    description "Fedora base with common dev tools"
    
    defaults {
        cpus 2
        memory "4g"
    }
}
"#;

        let doc: KdlDocument = kdl.parse().unwrap();
        let defs = parse_config_kdl(&doc).unwrap();

        assert_eq!(defs.len(), 1);
        let base = &defs["base"];
        assert_eq!(base.name, "base");
        assert_eq!(
            base.description.as_deref(),
            Some("Fedora base with common dev tools")
        );
        assert_eq!(base.defaults.as_ref().unwrap().cpus, Some(2));
        assert_eq!(
            base.defaults.as_ref().unwrap().memory.as_deref(),
            Some("4g")
        );
    }

    #[test]
    fn test_parse_container_with_parent() {
        let kdl = r#"
container "rust-dev" {
    parent "base"
}
"#;

        let doc: KdlDocument = kdl.parse().unwrap();
        let defs = parse_config_kdl(&doc).unwrap();

        let rust_dev = &defs["rust-dev"];
        assert_eq!(rust_dev.parent.as_deref(), Some("base"));
    }

    #[test]
    fn test_parse_container_with_mounts() {
        let kdl = r#"
container "base" {
    mounts {
        mount "workspace" target="/workspace" mode="rw"
        mount "binnacle" target="/binnacle" mode="rw"
        mount "cargo-cache" source="~/.cargo" target="/cargo-cache" mode="ro" optional=#true
    }
}
"#;

        let doc: KdlDocument = kdl.parse().unwrap();
        let defs = parse_config_kdl(&doc).unwrap();

        let base = &defs["base"];
        assert_eq!(base.mounts.len(), 3);

        assert_eq!(base.mounts[0].name, "workspace");
        assert_eq!(base.mounts[0].target, "/workspace");
        assert_eq!(base.mounts[0].mode, MountMode::ReadWrite);
        assert!(!base.mounts[0].optional);

        assert_eq!(base.mounts[2].name, "cargo-cache");
        assert_eq!(base.mounts[2].source.as_deref(), Some("~/.cargo"));
        assert_eq!(base.mounts[2].mode, MountMode::ReadOnly);
        assert!(base.mounts[2].optional);
    }

    #[test]
    fn test_reserved_name_rejected() {
        let kdl = r#"
container "binnacle" {
    description "Should be rejected"
}
"#;

        let doc: KdlDocument = kdl.parse().unwrap();
        let result = parse_config_kdl(&doc);

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        // The new error format uses "bn: error: config: reserved container name"
        assert!(
            err_msg.contains("reserved container name"),
            "Error message should mention reserved name: {}",
            err_msg
        );
        // Verify suggestion is included
        assert!(
            err_msg.contains("Choose a different name"),
            "Error message should include suggestion: {}",
            err_msg
        );
    }

    #[test]
    fn test_invalid_mount_mode() {
        let kdl = r#"
container "test" {
    mounts {
        mount "test" target="/test" mode="invalid"
    }
}
"#;

        let doc: KdlDocument = kdl.parse().unwrap();
        let result = parse_config_kdl(&doc);

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        // The new error format uses "bn: error: config: invalid mount mode"
        assert!(
            err_msg.contains("invalid mount mode"),
            "Error message should mention invalid mount mode: {}",
            err_msg
        );
    }

    #[test]
    fn test_multiple_containers() {
        let kdl = r#"
container "base" {
    description "Base container"
    defaults {
        cpus 2
    }
}

container "rust-dev" {
    parent "base"
    description "Rust development"
}
"#;

        let doc: KdlDocument = kdl.parse().unwrap();
        let defs = parse_config_kdl(&doc).unwrap();

        assert_eq!(defs.len(), 2);
        assert!(defs.contains_key("base"));
        assert!(defs.contains_key("rust-dev"));
    }

    #[test]
    fn test_mount_modes() {
        assert_eq!("ro".parse::<MountMode>().unwrap(), MountMode::ReadOnly);
        assert_eq!("rw".parse::<MountMode>().unwrap(), MountMode::ReadWrite);
        assert!("invalid".parse::<MountMode>().is_err());
    }

    #[test]
    #[serial_test::serial]
    fn test_discover_definitions_empty_returns_embedded() {
        use tempfile::TempDir;

        let temp = TempDir::new().unwrap();
        let repo_path = temp.path();

        // No config files exist, should return embedded fallback (both binnacle and default)
        let defs = super::discover_definitions(repo_path).unwrap();

        assert_eq!(
            defs.len(),
            2,
            "Expected 2 embedded definitions, got {}: {:?}",
            defs.len(),
            defs.iter().map(|d| &d.definition.name).collect::<Vec<_>>()
        );

        // Check that binnacle (worker) is included
        let binnacle_def = defs.iter().find(|d| d.definition.name == RESERVED_NAME);
        assert!(
            binnacle_def.is_some(),
            "Expected embedded binnacle definition"
        );
        let binnacle_def = binnacle_def.unwrap();
        assert_eq!(binnacle_def.source, super::DefinitionSource::Embedded);
        assert_eq!(
            binnacle_def.definition.parent,
            Some(EMBEDDED_DEFAULT_NAME.to_string()),
            "Embedded binnacle should derive from default"
        );

        // Check that default is included
        let default_def = defs
            .iter()
            .find(|d| d.definition.name == EMBEDDED_DEFAULT_NAME);
        assert!(
            default_def.is_some(),
            "Expected embedded default definition"
        );
        assert_eq!(
            default_def.unwrap().source,
            super::DefinitionSource::Embedded
        );
    }

    #[test]
    #[serial_test::serial]
    fn test_discover_definitions_project_only() {
        use std::fs;
        use tempfile::TempDir;

        let temp = TempDir::new().unwrap();
        let repo_path = temp.path();

        // Create project-level config
        let containers_dir = repo_path.join(".binnacle").join("containers");
        fs::create_dir_all(&containers_dir).unwrap();
        let config_path = containers_dir.join("config.kdl");
        fs::write(
            &config_path,
            r#"
container "base" {
    description "Project base container"
}
"#,
        )
        .unwrap();

        let defs = super::discover_definitions(repo_path).unwrap();

        // Should have 3 definitions: 1 project + 2 embedded (binnacle, default)
        assert_eq!(
            defs.len(),
            3,
            "Expected 3 definitions (1 project + 2 embedded), got {}: {:?}",
            defs.len(),
            defs.iter().map(|d| &d.definition.name).collect::<Vec<_>>()
        );
        assert_eq!(defs[0].definition.name, "base");
        assert_eq!(defs[0].source, super::DefinitionSource::Project);
        assert_eq!(defs[0].config_path, config_path);

        // Verify embedded definitions are included
        let embedded_count = defs
            .iter()
            .filter(|d| d.source == super::DefinitionSource::Embedded)
            .count();
        assert_eq!(embedded_count, 2, "Expected 2 embedded definitions");
    }

    #[test]
    #[serial_test::serial]
    fn test_discover_definitions_host_only() {
        use std::fs;
        use tempfile::TempDir;

        let temp = TempDir::new().unwrap();
        let repo_path = temp.path();

        // Create a data directory for host-level config
        let data_temp = TempDir::new().unwrap();

        // We need to use get_storage_dir to get the correct hash-based path
        // For testing, we'll manually construct the path
        let storage_dir =
            crate::storage::get_storage_dir_with_base(repo_path, data_temp.path()).unwrap();

        let containers_dir = storage_dir.join("containers");
        fs::create_dir_all(&containers_dir).unwrap();
        let config_path = containers_dir.join("config.kdl");
        fs::write(
            &config_path,
            r#"
container "my-dev" {
    description "User dev container"
}
"#,
        )
        .unwrap();

        // Use the DI variant for testing
        // SAFETY: This is a test, single-threaded execution
        unsafe {
            std::env::set_var("BN_DATA_DIR", data_temp.path());
        }
        let defs = super::discover_definitions(repo_path).unwrap();
        unsafe {
            std::env::remove_var("BN_DATA_DIR");
        }

        // Should have 3 definitions: 1 host + 2 embedded (binnacle, default)
        assert_eq!(
            defs.len(),
            3,
            "Expected 3 definitions (1 host + 2 embedded), got {}: {:?}",
            defs.len(),
            defs.iter().map(|d| &d.definition.name).collect::<Vec<_>>()
        );
        assert_eq!(defs[0].definition.name, "my-dev");
        assert_eq!(defs[0].source, super::DefinitionSource::Host);

        // Verify embedded definitions are included
        let embedded_count = defs
            .iter()
            .filter(|d| d.source == super::DefinitionSource::Embedded)
            .count();
        assert_eq!(embedded_count, 2, "Expected 2 embedded definitions");
    }

    #[test]
    #[serial_test::serial]
    fn test_discover_definitions_both_sources() {
        use std::fs;
        use tempfile::TempDir;

        // Create separate temp directories
        let repo_temp = TempDir::new().unwrap();
        let repo_path = repo_temp.path();

        // Create project-level config
        let project_containers = repo_path.join(".binnacle").join("containers");
        fs::create_dir_all(&project_containers).unwrap();
        fs::write(
            project_containers.join("config.kdl"),
            r#"
container "base" {
    description "Project base"
}
container "rust-dev" {
    description "Project Rust dev"
}
"#,
        )
        .unwrap();

        // Create host-level config
        let data_temp = TempDir::new().unwrap();
        let storage_dir =
            crate::storage::get_storage_dir_with_base(repo_path, data_temp.path()).unwrap();
        let host_containers = storage_dir.join("containers");
        fs::create_dir_all(&host_containers).unwrap();
        fs::write(
            host_containers.join("config.kdl"),
            r#"
container "my-env" {
    description "User environment"
}
container "rust-dev" {
    description "User Rust dev (conflict)"
}
"#,
        )
        .unwrap();

        // SAFETY: This is a test, single-threaded execution with serial_test
        unsafe {
            std::env::set_var("BN_DATA_DIR", data_temp.path());
        }
        let defs = super::discover_definitions(repo_path).unwrap();
        unsafe {
            std::env::remove_var("BN_DATA_DIR");
        }

        // Should have 6 definitions total (2 from project + 2 from host + 2 embedded)
        // Including the "rust-dev" conflict
        assert_eq!(
            defs.len(),
            6,
            "Expected 6 definitions (2 project + 2 host + 2 embedded), got {}: {:?}",
            defs.len(),
            defs.iter().map(|d| &d.definition.name).collect::<Vec<_>>()
        );

        // Check that we have all sources represented
        let project_count = defs
            .iter()
            .filter(|d| d.source == super::DefinitionSource::Project)
            .count();
        let host_count = defs
            .iter()
            .filter(|d| d.source == super::DefinitionSource::Host)
            .count();
        let embedded_count = defs
            .iter()
            .filter(|d| d.source == super::DefinitionSource::Embedded)
            .count();
        assert_eq!(project_count, 2);
        assert_eq!(host_count, 2);
        assert_eq!(embedded_count, 2, "Expected 2 embedded definitions");

        // Verify rust-dev appears twice (conflict)
        let rust_dev_count = defs
            .iter()
            .filter(|d| d.definition.name == "rust-dev")
            .count();
        assert_eq!(rust_dev_count, 2);
    }

    #[test]
    fn test_resolve_mount_source_special_workspace() {
        let repo_root = Path::new("/repo");
        let result = super::resolve_mount_source("workspace", repo_root).unwrap();
        assert_eq!(result, PathBuf::from("workspace"));
    }

    #[test]
    fn test_resolve_mount_source_special_binnacle() {
        let repo_root = Path::new("/repo");
        let result = super::resolve_mount_source("binnacle", repo_root).unwrap();
        assert_eq!(result, PathBuf::from("binnacle"));
    }

    #[test]
    fn test_resolve_mount_source_absolute() {
        let repo_root = Path::new("/repo");
        let result = super::resolve_mount_source("/usr/local/bin", repo_root).unwrap();
        assert_eq!(result, PathBuf::from("/usr/local/bin"));
    }

    #[test]
    fn test_resolve_mount_source_relative() {
        let repo_root = Path::new("/repo");
        let result = super::resolve_mount_source("data/cache", repo_root).unwrap();
        assert_eq!(result, PathBuf::from("/repo/data/cache"));
    }

    #[test]
    fn test_resolve_mount_source_tilde() {
        let repo_root = Path::new("/repo");
        let result = super::resolve_mount_source("~/.cargo", repo_root).unwrap();
        let home = dirs::home_dir().unwrap();
        assert_eq!(result, home.join(".cargo"));
    }

    #[test]
    fn test_resolve_mount_source_tilde_only() {
        let repo_root = Path::new("/repo");
        let result = super::resolve_mount_source("~", repo_root).unwrap();
        let home = dirs::home_dir().unwrap();
        assert_eq!(result, home);
    }

    #[test]
    fn test_resolve_mount_source_home_env() {
        let repo_root = Path::new("/repo");
        let result = super::resolve_mount_source("$HOME/.config", repo_root).unwrap();
        let home = dirs::home_dir().unwrap();
        assert_eq!(result, home.join(".config"));
    }

    #[test]
    fn test_resolve_mount_source_home_env_only() {
        let repo_root = Path::new("/repo");
        let result = super::resolve_mount_source("$HOME", repo_root).unwrap();
        let home = dirs::home_dir().unwrap();
        assert_eq!(result, home);
    }

    #[test]
    fn test_resolve_mount_source_relative_with_dots() {
        let repo_root = Path::new("/repo");
        let result = super::resolve_mount_source("../sibling/data", repo_root).unwrap();
        assert_eq!(result, PathBuf::from("/repo/../sibling/data"));
    }

    #[test]
    fn test_compute_repo_hash() {
        use tempfile::TempDir;

        let temp = TempDir::new().unwrap();
        let hash = super::compute_repo_hash(temp.path()).unwrap();

        // Should be 12 hex characters
        assert_eq!(hash.len(), 12);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_generate_image_name() {
        use tempfile::TempDir;

        let temp = TempDir::new().unwrap();
        let image = super::generate_image_name(temp.path(), "rust-dev").unwrap();

        // Should start with localhost/bn- and end with :latest
        assert!(image.starts_with("localhost/bn-"));
        assert!(image.ends_with("-rust-dev:latest"));

        // Extract hash part (between "bn-" and "-rust-dev")
        let parts: Vec<&str> = image.split('-').collect();
        assert!(parts.len() >= 3);
        let hash_part = parts[1];
        assert_eq!(hash_part.len(), 12);
    }

    #[test]
    fn test_generate_image_name_for_definition_embedded() {
        use tempfile::TempDir;

        let temp = TempDir::new().unwrap();

        // Embedded binnacle (worker) uses hardcoded name
        let image =
            super::generate_image_name_for_definition(temp.path(), "binnacle", "latest", true)
                .unwrap();
        assert_eq!(image, "localhost/binnacle-worker:latest");

        // Embedded default uses hardcoded name
        let image =
            super::generate_image_name_for_definition(temp.path(), "default", "latest", true)
                .unwrap();
        assert_eq!(image, "localhost/binnacle-default:latest");

        // Custom tag works
        let image =
            super::generate_image_name_for_definition(temp.path(), "binnacle", "v1.0", true)
                .unwrap();
        assert_eq!(image, "localhost/binnacle-worker:v1.0");
    }

    #[test]
    fn test_generate_image_name_for_definition_custom() {
        use tempfile::TempDir;

        let temp = TempDir::new().unwrap();

        // Custom definition uses repository-scoped name
        let image =
            super::generate_image_name_for_definition(temp.path(), "rust-dev", "latest", false)
                .unwrap();
        assert!(image.starts_with("localhost/bn-"));
        assert!(image.ends_with("-rust-dev:latest"));

        // Custom tag works
        let image =
            super::generate_image_name_for_definition(temp.path(), "rust-dev", "v2.0", false)
                .unwrap();
        assert!(image.starts_with("localhost/bn-"));
        assert!(image.ends_with("-rust-dev:v2.0"));
    }

    #[test]
    fn test_generate_image_name_for_definition_unknown_embedded() {
        use tempfile::TempDir;

        let temp = TempDir::new().unwrap();

        // Unknown embedded definition should error
        let result =
            super::generate_image_name_for_definition(temp.path(), "unknown", "latest", true);
        assert!(result.is_err());
    }

    #[test]
    fn test_compute_build_order_no_dependencies() {
        let mut defs = HashMap::new();
        defs.insert(
            "base".to_string(),
            ContainerDefinition {
                name: "base".to_string(),
                description: None,
                parent: None,
                defaults: None,
                mounts: vec![],
            },
        );
        defs.insert(
            "app".to_string(),
            ContainerDefinition {
                name: "app".to_string(),
                description: None,
                parent: None,
                defaults: None,
                mounts: vec![],
            },
        );

        let order = super::compute_build_order(&defs).unwrap();
        assert_eq!(order.len(), 2);
        assert!(order.contains(&"base".to_string()));
        assert!(order.contains(&"app".to_string()));
    }

    #[test]
    fn test_compute_build_order_linear_chain() {
        let mut defs = HashMap::new();
        defs.insert(
            "base".to_string(),
            ContainerDefinition {
                name: "base".to_string(),
                description: None,
                parent: None,
                defaults: None,
                mounts: vec![],
            },
        );
        defs.insert(
            "rust-dev".to_string(),
            ContainerDefinition {
                name: "rust-dev".to_string(),
                description: None,
                parent: Some("base".to_string()),
                defaults: None,
                mounts: vec![],
            },
        );
        defs.insert(
            "app".to_string(),
            ContainerDefinition {
                name: "app".to_string(),
                description: None,
                parent: Some("rust-dev".to_string()),
                defaults: None,
                mounts: vec![],
            },
        );

        let order = super::compute_build_order(&defs).unwrap();
        assert_eq!(order.len(), 3);

        // base must come before rust-dev, rust-dev before app
        let base_idx = order.iter().position(|s| s == "base").unwrap();
        let rust_idx = order.iter().position(|s| s == "rust-dev").unwrap();
        let app_idx = order.iter().position(|s| s == "app").unwrap();

        assert!(base_idx < rust_idx);
        assert!(rust_idx < app_idx);
    }

    #[test]
    fn test_compute_build_order_tree() {
        let mut defs = HashMap::new();
        defs.insert(
            "base".to_string(),
            ContainerDefinition {
                name: "base".to_string(),
                description: None,
                parent: None,
                defaults: None,
                mounts: vec![],
            },
        );
        defs.insert(
            "rust-dev".to_string(),
            ContainerDefinition {
                name: "rust-dev".to_string(),
                description: None,
                parent: Some("base".to_string()),
                defaults: None,
                mounts: vec![],
            },
        );
        defs.insert(
            "python-dev".to_string(),
            ContainerDefinition {
                name: "python-dev".to_string(),
                description: None,
                parent: Some("base".to_string()),
                defaults: None,
                mounts: vec![],
            },
        );

        let order = super::compute_build_order(&defs).unwrap();
        assert_eq!(order.len(), 3);

        // base must come first
        assert_eq!(order[0], "base");

        // rust-dev and python-dev can be in either order, but both after base
        assert!(order[1..].contains(&"rust-dev".to_string()));
        assert!(order[1..].contains(&"python-dev".to_string()));
    }

    #[test]
    fn test_compute_build_order_circular_dependency() {
        let mut defs = HashMap::new();
        defs.insert(
            "a".to_string(),
            ContainerDefinition {
                name: "a".to_string(),
                description: None,
                parent: Some("b".to_string()),
                defaults: None,
                mounts: vec![],
            },
        );
        defs.insert(
            "b".to_string(),
            ContainerDefinition {
                name: "b".to_string(),
                description: None,
                parent: Some("a".to_string()),
                defaults: None,
                mounts: vec![],
            },
        );

        let result = super::compute_build_order(&defs);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        // The new error format uses "bn: error: config: circular dependency detected"
        assert!(
            err_msg.contains("circular dependency"),
            "Error message should mention circular dependency: {}",
            err_msg
        );
    }

    #[test]
    fn test_compute_build_order_missing_parent() {
        let mut defs = HashMap::new();
        defs.insert(
            "child".to_string(),
            ContainerDefinition {
                name: "child".to_string(),
                description: None,
                parent: Some("nonexistent".to_string()),
                defaults: None,
                mounts: vec![],
            },
        );

        let result = super::compute_build_order(&defs);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        // The new error format uses "bn: error: build: parent container not found"
        assert!(
            err_msg.contains("parent container not found"),
            "Error message should mention parent not found: {}",
            err_msg
        );
    }

    #[test]
    fn test_compute_build_order_complex_graph() {
        let mut defs = HashMap::new();

        // Create a diamond dependency graph:
        //       base
        //      /    \
        //   left    right
        //      \    /
        //       bottom

        defs.insert(
            "base".to_string(),
            ContainerDefinition {
                name: "base".to_string(),
                description: None,
                parent: None,
                defaults: None,
                mounts: vec![],
            },
        );
        defs.insert(
            "left".to_string(),
            ContainerDefinition {
                name: "left".to_string(),
                description: None,
                parent: Some("base".to_string()),
                defaults: None,
                mounts: vec![],
            },
        );
        defs.insert(
            "right".to_string(),
            ContainerDefinition {
                name: "right".to_string(),
                description: None,
                parent: Some("base".to_string()),
                defaults: None,
                mounts: vec![],
            },
        );
        // bottom can only depend on one parent in our model, so this creates a valid tree
        defs.insert(
            "bottom".to_string(),
            ContainerDefinition {
                name: "bottom".to_string(),
                description: None,
                parent: Some("left".to_string()),
                defaults: None,
                mounts: vec![],
            },
        );

        let order = super::compute_build_order(&defs).unwrap();
        assert_eq!(order.len(), 4);

        // Verify ordering constraints
        let base_idx = order.iter().position(|s| s == "base").unwrap();
        let left_idx = order.iter().position(|s| s == "left").unwrap();
        let right_idx = order.iter().position(|s| s == "right").unwrap();
        let bottom_idx = order.iter().position(|s| s == "bottom").unwrap();

        assert!(base_idx < left_idx);
        assert!(base_idx < right_idx);
        assert!(left_idx < bottom_idx);
    }

    #[test]
    fn test_validate_mounts_special_mounts() {
        use tempfile::TempDir;
        let temp = TempDir::new().unwrap();

        let mounts = vec![
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
        ];

        let result = super::validate_mounts(&mounts, temp.path()).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].resolved_source, PathBuf::from("workspace"));
        assert_eq!(result[1].resolved_source, PathBuf::from("binnacle"));
    }

    #[test]
    fn test_validate_mounts_required_missing_fails() {
        use tempfile::TempDir;
        let temp = TempDir::new().unwrap();

        let mounts = vec![Mount {
            name: "required".to_string(),
            source: Some("/nonexistent/path".to_string()),
            target: "/target".to_string(),
            mode: MountMode::ReadWrite,
            optional: false,
        }];

        let result = super::validate_mounts(&mounts, temp.path());
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        // The new error format uses "bn: error: mount: source path not found"
        assert!(
            err_msg.contains("source path not found"),
            "Error message should mention source path not found: {}",
            err_msg
        );
    }

    #[test]
    fn test_validate_mounts_optional_missing_skipped() {
        use tempfile::TempDir;
        let temp = TempDir::new().unwrap();

        let mounts = vec![Mount {
            name: "optional-cache".to_string(),
            source: Some("/nonexistent/cache".to_string()),
            target: "/cache".to_string(),
            mode: MountMode::ReadOnly,
            optional: true,
        }];

        let result = super::validate_mounts(&mounts, temp.path()).unwrap();
        // Optional mount should be skipped if missing
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_validate_mounts_optional_exists_included() {
        use std::fs;
        use tempfile::TempDir;
        let temp = TempDir::new().unwrap();

        // Create a test directory
        let cache_dir = temp.path().join("cache");
        fs::create_dir(&cache_dir).unwrap();

        let mounts = vec![Mount {
            name: "optional-cache".to_string(),
            source: Some(cache_dir.to_string_lossy().to_string()),
            target: "/cache".to_string(),
            mode: MountMode::ReadOnly,
            optional: true,
        }];

        let result = super::validate_mounts(&mounts, temp.path()).unwrap();
        // Optional mount should be included if it exists
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].mount.name, "optional-cache");
        assert_eq!(result[0].resolved_source, cache_dir);
    }

    #[test]
    fn test_validate_mounts_mixed_required_and_optional() {
        use std::fs;
        use tempfile::TempDir;
        let temp = TempDir::new().unwrap();

        // Create required directory
        let data_dir = temp.path().join("data");
        fs::create_dir(&data_dir).unwrap();

        let mounts = vec![
            Mount {
                name: "required-data".to_string(),
                source: Some(data_dir.to_string_lossy().to_string()),
                target: "/data".to_string(),
                mode: MountMode::ReadWrite,
                optional: false,
            },
            Mount {
                name: "optional-cache".to_string(),
                source: Some("/nonexistent/cache".to_string()),
                target: "/cache".to_string(),
                mode: MountMode::ReadOnly,
                optional: true,
            },
            Mount {
                name: "workspace".to_string(),
                source: Some("workspace".to_string()),
                target: "/workspace".to_string(),
                mode: MountMode::ReadWrite,
                optional: false,
            },
        ];

        let result = super::validate_mounts(&mounts, temp.path()).unwrap();
        // Should include required data and workspace, skip optional cache
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].mount.name, "required-data");
        assert_eq!(result[1].mount.name, "workspace");
    }

    #[test]
    fn test_validate_mounts_relative_path() {
        use std::fs;
        use tempfile::TempDir;
        let temp = TempDir::new().unwrap();

        // Create a relative directory
        let cache_dir = temp.path().join("cache");
        fs::create_dir(&cache_dir).unwrap();

        let mounts = vec![Mount {
            name: "cache".to_string(),
            source: Some("cache".to_string()),
            target: "/cache".to_string(),
            mode: MountMode::ReadWrite,
            optional: false,
        }];

        let result = super::validate_mounts(&mounts, temp.path()).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].resolved_source, cache_dir);
    }

    #[test]
    fn test_validate_mounts_no_source() {
        use tempfile::TempDir;
        let temp = TempDir::new().unwrap();

        // Mount with no source (runtime-provided)
        let mounts = vec![Mount {
            name: "runtime".to_string(),
            source: None,
            target: "/runtime".to_string(),
            mode: MountMode::ReadWrite,
            optional: false,
        }];

        let result = super::validate_mounts(&mounts, temp.path()).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].resolved_source, PathBuf::from(""));
    }

    #[test]
    fn test_get_entrypoint_path() {
        let mut defs = HashMap::new();

        let base = ContainerDefinition {
            name: "base".to_string(),
            description: None,
            parent: None,
            defaults: None,
            mounts: vec![],
        };
        defs.insert("base".to_string(), base.clone());

        let path = super::get_entrypoint_path(&base, &defs);
        assert_eq!(path, "/entrypoint.sh");
    }

    // === Conflict Detection Tests ===

    #[test]
    fn test_detect_conflicts_no_conflict() {
        let defs = vec![
            super::DefinitionWithSource {
                definition: ContainerDefinition {
                    name: "base".to_string(),
                    description: Some("Base definition".to_string()),
                    parent: None,
                    defaults: None,
                    mounts: vec![],
                },
                source: super::DefinitionSource::Project,
                config_path: std::path::PathBuf::from("/project/.binnacle/containers/config.kdl"),
                modified_at: None,
            },
            super::DefinitionWithSource {
                definition: ContainerDefinition {
                    name: "dev".to_string(),
                    description: Some("Dev definition".to_string()),
                    parent: None,
                    defaults: None,
                    mounts: vec![],
                },
                source: super::DefinitionSource::Host,
                config_path: std::path::PathBuf::from(
                    "/home/user/.local/share/binnacle/abc123/containers/config.kdl",
                ),
                modified_at: None,
            },
        ];

        let conflicts = super::detect_conflicts(&defs);
        assert!(conflicts.is_empty());
    }

    #[test]
    fn test_detect_conflicts_with_conflict() {
        let defs = vec![
            super::DefinitionWithSource {
                definition: ContainerDefinition {
                    name: "worker".to_string(),
                    description: Some("Project worker".to_string()),
                    parent: None,
                    defaults: None,
                    mounts: vec![],
                },
                source: super::DefinitionSource::Project,
                config_path: std::path::PathBuf::from("/project/.binnacle/containers/config.kdl"),
                modified_at: None,
            },
            super::DefinitionWithSource {
                definition: ContainerDefinition {
                    name: "worker".to_string(),
                    description: Some("Host worker".to_string()),
                    parent: None,
                    defaults: None,
                    mounts: vec![],
                },
                source: super::DefinitionSource::Host,
                config_path: std::path::PathBuf::from(
                    "/home/user/.local/share/binnacle/abc123/containers/config.kdl",
                ),
                modified_at: None,
            },
        ];

        let conflicts = super::detect_conflicts(&defs);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].name, "worker");
        assert_eq!(
            conflicts[0].project_def.source,
            super::DefinitionSource::Project
        );
        assert_eq!(conflicts[0].host_def.source, super::DefinitionSource::Host);
    }

    #[test]
    fn test_resolve_definition_no_conflict() {
        let defs = vec![super::DefinitionWithSource {
            definition: ContainerDefinition {
                name: "worker".to_string(),
                description: Some("Only worker".to_string()),
                parent: None,
                defaults: None,
                mounts: vec![],
            },
            source: super::DefinitionSource::Project,
            config_path: std::path::PathBuf::from("/project/.binnacle/containers/config.kdl"),
            modified_at: None,
        }];

        // Should work with no preference when there's no conflict
        let resolved = super::resolve_definition(&defs, "worker", super::SourcePreference::None);
        assert!(resolved.is_ok());
        assert_eq!(resolved.unwrap().definition.name, "worker");
    }

    #[test]
    fn test_resolve_definition_conflict_no_preference_errors() {
        let defs = vec![
            super::DefinitionWithSource {
                definition: ContainerDefinition {
                    name: "worker".to_string(),
                    description: Some("Project worker".to_string()),
                    parent: None,
                    defaults: None,
                    mounts: vec![],
                },
                source: super::DefinitionSource::Project,
                config_path: std::path::PathBuf::from("/project/.binnacle/containers/config.kdl"),
                modified_at: None,
            },
            super::DefinitionWithSource {
                definition: ContainerDefinition {
                    name: "worker".to_string(),
                    description: Some("Host worker".to_string()),
                    parent: None,
                    defaults: None,
                    mounts: vec![],
                },
                source: super::DefinitionSource::Host,
                config_path: std::path::PathBuf::from(
                    "/home/user/.local/share/binnacle/abc123/containers/config.kdl",
                ),
                modified_at: None,
            },
        ];

        // Should error when there's a conflict and no preference
        let resolved = super::resolve_definition(&defs, "worker", super::SourcePreference::None);
        assert!(resolved.is_err());
        let err = resolved.unwrap_err().to_string();
        assert!(err.contains("Conflict"));
        assert!(err.contains("--project or --host"));
    }

    #[test]
    fn test_resolve_definition_conflict_with_project_preference() {
        let defs = vec![
            super::DefinitionWithSource {
                definition: ContainerDefinition {
                    name: "worker".to_string(),
                    description: Some("Project worker".to_string()),
                    parent: None,
                    defaults: None,
                    mounts: vec![],
                },
                source: super::DefinitionSource::Project,
                config_path: std::path::PathBuf::from("/project/.binnacle/containers/config.kdl"),
                modified_at: None,
            },
            super::DefinitionWithSource {
                definition: ContainerDefinition {
                    name: "worker".to_string(),
                    description: Some("Host worker".to_string()),
                    parent: None,
                    defaults: None,
                    mounts: vec![],
                },
                source: super::DefinitionSource::Host,
                config_path: std::path::PathBuf::from(
                    "/home/user/.local/share/binnacle/abc123/containers/config.kdl",
                ),
                modified_at: None,
            },
        ];

        // Should resolve to project definition with Project preference
        let resolved = super::resolve_definition(&defs, "worker", super::SourcePreference::Project);
        assert!(resolved.is_ok());
        let def = resolved.unwrap();
        assert_eq!(
            def.definition.description,
            Some("Project worker".to_string())
        );
        assert_eq!(def.source, super::DefinitionSource::Project);
    }

    #[test]
    fn test_resolve_definition_conflict_with_host_preference() {
        let defs = vec![
            super::DefinitionWithSource {
                definition: ContainerDefinition {
                    name: "worker".to_string(),
                    description: Some("Project worker".to_string()),
                    parent: None,
                    defaults: None,
                    mounts: vec![],
                },
                source: super::DefinitionSource::Project,
                config_path: std::path::PathBuf::from("/project/.binnacle/containers/config.kdl"),
                modified_at: None,
            },
            super::DefinitionWithSource {
                definition: ContainerDefinition {
                    name: "worker".to_string(),
                    description: Some("Host worker".to_string()),
                    parent: None,
                    defaults: None,
                    mounts: vec![],
                },
                source: super::DefinitionSource::Host,
                config_path: std::path::PathBuf::from(
                    "/home/user/.local/share/binnacle/abc123/containers/config.kdl",
                ),
                modified_at: None,
            },
        ];

        // Should resolve to host definition with Host preference
        let resolved = super::resolve_definition(&defs, "worker", super::SourcePreference::Host);
        assert!(resolved.is_ok());
        let def = resolved.unwrap();
        assert_eq!(def.definition.description, Some("Host worker".to_string()));
        assert_eq!(def.source, super::DefinitionSource::Host);
    }

    #[test]
    fn test_resolve_definition_not_found() {
        let defs = vec![super::DefinitionWithSource {
            definition: ContainerDefinition {
                name: "worker".to_string(),
                description: Some("Worker".to_string()),
                parent: None,
                defaults: None,
                mounts: vec![],
            },
            source: super::DefinitionSource::Project,
            config_path: std::path::PathBuf::from("/project/.binnacle/containers/config.kdl"),
            modified_at: None,
        }];

        // Should error for non-existent definition
        let resolved =
            super::resolve_definition(&defs, "nonexistent", super::SourcePreference::None);
        assert!(resolved.is_err());
        let err = resolved.unwrap_err().to_string();
        assert!(err.contains("not found"));
    }

    #[test]
    fn test_conflict_display_includes_metadata() {
        let conflict = super::DefinitionConflict {
            name: "worker".to_string(),
            project_def: super::DefinitionWithSource {
                definition: ContainerDefinition {
                    name: "worker".to_string(),
                    description: Some("Project worker definition".to_string()),
                    parent: None,
                    defaults: None,
                    mounts: vec![],
                },
                source: super::DefinitionSource::Project,
                config_path: std::path::PathBuf::from("/project/.binnacle/containers/config.kdl"),
                modified_at: Some(
                    chrono::DateTime::parse_from_rfc3339("2024-01-15T10:30:00Z")
                        .unwrap()
                        .with_timezone(&chrono::Utc),
                ),
            },
            host_def: super::DefinitionWithSource {
                definition: ContainerDefinition {
                    name: "worker".to_string(),
                    description: Some("Host worker definition".to_string()),
                    parent: None,
                    defaults: None,
                    mounts: vec![],
                },
                source: super::DefinitionSource::Host,
                config_path: std::path::PathBuf::from(
                    "/home/user/.local/share/binnacle/abc123/containers/config.kdl",
                ),
                modified_at: Some(
                    chrono::DateTime::parse_from_rfc3339("2024-01-20T15:45:00Z")
                        .unwrap()
                        .with_timezone(&chrono::Utc),
                ),
            },
        };

        let display = format!("{}", conflict);
        assert!(display.contains("Conflict: definition 'worker'"));
        assert!(display.contains("PROJECT"));
        assert!(display.contains("HOST"));
        assert!(display.contains("Project worker definition"));
        assert!(display.contains("Host worker definition"));
        assert!(display.contains("Modified:"));
        assert!(display.contains("--project or --host"));
    }
}
