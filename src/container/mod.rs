//! Container definition management and config.kdl parsing.

use crate::{Error, Result};
use kdl::{KdlDocument, KdlNode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Reserved container name that cannot be used by projects/users.
pub const RESERVED_NAME: &str = "binnacle";

/// Container definition parsed from config.kdl
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContainerDefinition {
    pub name: String,
    pub description: Option<String>,
    pub parent: Option<String>,
    pub entrypoint_mode: EntrypointMode,
    pub defaults: Option<Defaults>,
    pub mounts: Vec<Mount>,
}

/// Entrypoint chaining mode
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EntrypointMode {
    /// Child's entrypoint replaces parent's (default)
    #[default]
    Replace,
    /// Child runs first, then exec's parent's entrypoint
    Before,
    /// Parent runs first, then child's runs
    After,
}

impl std::str::FromStr for EntrypointMode {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "replace" => Ok(Self::Replace),
            "before" => Ok(Self::Before),
            "after" => Ok(Self::After),
            _ => Err(Error::Other(format!(
                "Invalid entrypoint mode: '{}' (expected 'replace', 'before', or 'after')",
                s
            ))),
        }
    }
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
            _ => Err(Error::Other(format!(
                "Invalid mount mode: '{}' (expected 'ro' or 'rw')",
                s
            ))),
        }
    }
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
            return Err(Error::Other(format!(
                "Container name '{}' is reserved and cannot be used",
                RESERVED_NAME
            )));
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
        .ok_or_else(|| Error::Other("Container node must have a name argument".to_string()))?
        .to_string();

    let mut description = None;
    let mut parent = None;
    let mut entrypoint_mode = EntrypointMode::default();
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
                "entrypoint" => {
                    if let Some(mode_str) = child.get("mode").and_then(|v| v.as_string()) {
                        entrypoint_mode = mode_str.parse()?;
                    }
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
        entrypoint_mode,
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
        .ok_or_else(|| Error::Other("Mount node must have a name argument".to_string()))?
        .to_string();

    // Get target (required)
    let target = node
        .get("target")
        .and_then(|v| v.as_string())
        .ok_or_else(|| Error::Other(format!("Mount '{}' must have a target", name)))?
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
        assert_eq!(base.entrypoint_mode, EntrypointMode::Replace);
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
    entrypoint mode="before"
}
"#;

        let doc: KdlDocument = kdl.parse().unwrap();
        let defs = parse_config_kdl(&doc).unwrap();

        let rust_dev = &defs["rust-dev"];
        assert_eq!(rust_dev.parent.as_deref(), Some("base"));
        assert_eq!(rust_dev.entrypoint_mode, EntrypointMode::Before);
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
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("reserved and cannot be used")
        );
    }

    #[test]
    fn test_invalid_entrypoint_mode() {
        let kdl = r#"
container "test" {
    entrypoint mode="invalid"
}
"#;

        let doc: KdlDocument = kdl.parse().unwrap();
        let result = parse_config_kdl(&doc);

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Invalid entrypoint mode")
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
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Invalid mount mode")
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
    entrypoint mode="before"
}
"#;

        let doc: KdlDocument = kdl.parse().unwrap();
        let defs = parse_config_kdl(&doc).unwrap();

        assert_eq!(defs.len(), 2);
        assert!(defs.contains_key("base"));
        assert!(defs.contains_key("rust-dev"));
    }

    #[test]
    fn test_entrypoint_modes() {
        assert_eq!(
            "replace".parse::<EntrypointMode>().unwrap(),
            EntrypointMode::Replace
        );
        assert_eq!(
            "before".parse::<EntrypointMode>().unwrap(),
            EntrypointMode::Before
        );
        assert_eq!(
            "after".parse::<EntrypointMode>().unwrap(),
            EntrypointMode::After
        );
        assert!("invalid".parse::<EntrypointMode>().is_err());
    }

    #[test]
    fn test_mount_modes() {
        assert_eq!("ro".parse::<MountMode>().unwrap(), MountMode::ReadOnly);
        assert_eq!("rw".parse::<MountMode>().unwrap(), MountMode::ReadWrite);
        assert!("invalid".parse::<MountMode>().is_err());
    }
}
