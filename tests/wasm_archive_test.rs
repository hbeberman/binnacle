//! Integration tests for WASM archive loading
//!
//! Tests that the archive parser can load real .bng archives created by bn system store export.
//!
//! These tests require the `wasm` feature to be enabled.

#![cfg(feature = "wasm")]

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
