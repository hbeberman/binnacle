//! Integration tests for WASM archive loading
//!
//! Tests that the archive parser can load real .bng archives created by bn system store export.
//!
//! These tests require the `wasm` feature to be enabled.

#![cfg(feature = "wasm")]

use std::process::Command;
use tempfile::TempDir;

/// Get the bn binary path
fn bn_binary() -> std::path::PathBuf {
    #[allow(deprecated)]
    assert_cmd::cargo::cargo_bin("bn")
}

/// Test that GraphData can load a real .bng archive created by bn system store export
#[test]
fn test_load_real_bng_archive() {
    // Create a temp repo and initialize binnacle
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Initialize a git repo (required for binnacle)
    Command::new("git")
        .args(["init"])
        .current_dir(repo_path)
        .output()
        .expect("Failed to init git repo");

    // Get the bn binary path (from cargo)
    let bn_path = bn_binary();

    // Initialize binnacle
    Command::new(&bn_path)
        .args(["system", "init"])
        .current_dir(repo_path)
        .output()
        .expect("Failed to init binnacle");

    // Create some test data
    Command::new(&bn_path)
        .args(["task", "create", "Test task 1", "-p", "1"])
        .current_dir(repo_path)
        .output()
        .expect("Failed to create task");

    Command::new(&bn_path)
        .args(["task", "create", "Test task 2", "-p", "2"])
        .current_dir(repo_path)
        .output()
        .expect("Failed to create task");

    Command::new(&bn_path)
        .args(["bug", "create", "Test bug", "--severity", "high"])
        .current_dir(repo_path)
        .output()
        .expect("Failed to create bug");

    // Export to .bng archive
    let archive_path = repo_path.join("test.bng");
    let output = Command::new(&bn_path)
        .args(["system", "store", "export", archive_path.to_str().unwrap()])
        .current_dir(repo_path)
        .output()
        .expect("Failed to export archive");

    assert!(output.status.success(), "Export failed: {:?}", output);

    // Read the archive
    let data = std::fs::read(&archive_path).expect("Failed to read archive");
    assert!(data.len() > 100, "Archive too small: {} bytes", data.len());

    // Parse with GraphData
    use binnacle::wasm::GraphData;
    let graph = GraphData::from_archive_bytes(&data).expect("Failed to parse archive");

    // Verify we got the expected entities
    assert!(
        graph.entities.len() >= 3,
        "Expected at least 3 entities, got {}",
        graph.entities.len()
    );

    // Check we have tasks and bugs
    let task_count = graph
        .entities
        .iter()
        .filter(|e| e.entity_type == "task")
        .count();
    let bug_count = graph
        .entities
        .iter()
        .filter(|e| e.entity_type == "bug")
        .count();

    assert_eq!(task_count, 2, "Expected 2 tasks");
    assert_eq!(bug_count, 1, "Expected 1 bug");
}

/// Test that the archive manifest is parsed correctly
#[test]
fn test_archive_manifest_parsing() {
    // Create a temp repo and initialize binnacle
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Initialize a git repo
    Command::new("git")
        .args(["init"])
        .current_dir(repo_path)
        .output()
        .expect("Failed to init git repo");

    let bn_path = bn_binary();

    // Initialize binnacle
    Command::new(&bn_path)
        .args(["system", "init"])
        .current_dir(repo_path)
        .output()
        .expect("Failed to init binnacle");

    // Export to .bng archive
    let archive_path = repo_path.join("test.bng");
    Command::new(&bn_path)
        .args(["system", "store", "export", archive_path.to_str().unwrap()])
        .current_dir(repo_path)
        .output()
        .expect("Failed to export archive");

    // Read and parse
    let data = std::fs::read(&archive_path).expect("Failed to read archive");

    use binnacle::wasm::GraphData;
    let graph = GraphData::from_archive_bytes(&data).expect("Failed to parse archive");

    // Manifest should have version info
    assert!(
        graph.manifest.binnacle_version.is_some(),
        "Expected binnacle_version in manifest"
    );
    assert!(
        graph.manifest.exported_at.is_some(),
        "Expected exported_at in manifest"
    );
}

/// Test loading existing archive files from the archive/ directory
#[test]
fn test_load_existing_archives() {
    use binnacle::wasm::GraphData;

    // Find archive files
    let archive_dir = std::path::Path::new("archive");
    if !archive_dir.exists() {
        eprintln!("⚠ Archive directory doesn't exist, skipping test");
        return;
    }

    let mut tested = 0;
    for entry in std::fs::read_dir(archive_dir).expect("Failed to read archive dir") {
        let entry = entry.expect("Failed to read dir entry");
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) != Some("bng") {
            continue;
        }

        println!("\nTesting archive: {:?}", path.file_name().unwrap());
        let data = std::fs::read(&path).expect("Failed to read archive");
        println!("  Size: {} bytes", data.len());

        match GraphData::from_archive_bytes(&data) {
            Ok(graph) => {
                println!("  ✓ Loaded {} entities", graph.entities.len());
                println!("  ✓ Loaded {} edges", graph.edges.len());

                // Check for entities with empty titles
                for entity in &graph.entities {
                    if entity.title.is_empty() {
                        println!(
                            "  ⚠ Entity {} ({}) has empty title",
                            entity.id, entity.entity_type
                        );
                    }
                }
                tested += 1;
            }
            Err(e) => {
                panic!(
                    "Failed to load archive {:?}: {}",
                    path.file_name().unwrap(),
                    e
                );
            }
        }
    }

    assert!(tested > 0, "No archives were tested");
}

/// Test that test entities (TestNode) are parsed correctly with their "name" field
#[test]
fn test_parse_test_entities() {
    use binnacle::wasm::GraphData;

    // Find an archive with test entities
    let archive_path =
        std::path::Path::new("archive/bn_86b0e49432cdfc8849d46ac5ce453501cfddc1fb.bng");
    if !archive_path.exists() {
        eprintln!("⚠ Test archive doesn't exist, skipping test");
        return;
    }

    let data = std::fs::read(archive_path).expect("Failed to read archive");
    let graph = GraphData::from_archive_bytes(&data).expect("Failed to parse archive");

    // Find test entities
    let test_entities: Vec<_> = graph
        .entities
        .iter()
        .filter(|e| e.entity_type == "test")
        .collect();

    assert!(
        !test_entities.is_empty(),
        "Expected at least one test entity in archive"
    );

    // Verify test entities have non-empty titles (derived from "name" field)
    for test_entity in test_entities {
        assert!(
            !test_entity.title.is_empty(),
            "Test entity {} should have non-empty title (from name field)",
            test_entity.id
        );
    }
}
