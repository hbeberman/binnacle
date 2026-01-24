# Contributing to Binnacle

Thank you for contributing to binnacle! This guide will help you get started.

## Development Setup

1. **Clone and build**
   ```bash
   git clone https://github.com/yourusername/binnacle.git
   cd binnacle
   cargo build
   ```

2. **Install cargo-audit** (required for pre-commit hook)
   ```bash
   cargo install cargo-audit
   ```

3. **Enable git hooks**
   ```bash
   git config core.hooksPath hooks
   ```

   This configures git to use the tracked hooks in the `hooks/` directory, which includes a pre-commit hook that validates:
   - Code formatting (`cargo fmt --check`)
   - Linting (`cargo clippy -- -D warnings`)
   - Security vulnerabilities (`cargo audit`)

## Development Workflow

### Before Committing

The pre-commit hook will automatically run these checks, but you can run them manually:

```bash
# Run all checks
just check

# Run tests
just test

# Security audit
cargo audit

# Or individually
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

### Code Style

- Follow Rust standard formatting (enforced by `rustfmt`)
- Address all clippy warnings (CI fails on warnings)
- Write tests for new features
- Update documentation when adding/changing features

### Commit Messages

We use conventional commit style:
- `feat:` for new features
- `fix:` for bug fixes
- `docs:` for documentation changes
- `test:` for test additions/changes
- `refactor:` for code refactoring
- `chore:` for maintenance tasks

### Testing

Run the full test suite:
```bash
cargo test --all-features
```

For GUI-specific tests:
```bash
cargo test --features gui
```

### Using Binnacle to Track Binnacle Development

We dogfood! Use binnacle to track your work:

```bash
# See what's ready to work on
bn ready

# Claim a task
bn task update bn-xxxx --status in_progress

# Create linked tests
bn test create "Test feature X" --cmd "cargo test feature_x" --task bn-xxxx

# When done
bn task close bn-xxxx --reason "Implemented feature X"
```

## Pull Requests

1. Create a feature branch
2. Make your changes
3. Ensure all tests pass: `cargo test --all-features`
4. Ensure checks pass: `just check`
5. Push and open a PR
6. CI will automatically run tests, clippy, and format checks

## Questions?

Open an issue or reach out to the maintainers!

## Adding a New Entity Type

When adding a new entity type to binnacle (like Task, Bug, Idea, etc.), you must update multiple places. Use this checklist to avoid data loss bugs:

### Checklist

1. **Define the model** (`src/models/mod.rs`)
   - [ ] Create the struct with `entity_type` field
   - [ ] Implement serialization/deserialization
   - [ ] Add to `EntityType` enum if needed

2. **Update storage schema** (`src/storage/mod.rs`)
   - [ ] Add `CREATE TABLE` in `init_schema()`
   - [ ] Add `DELETE FROM` in `rebuild_cache()` clear section
   - [ ] Add re-read logic in `rebuild_cache()` for your `<entity>.jsonl`
   - [ ] Implement `cache_<entity>()` method
   - [ ] Implement CRUD methods (`create_<entity>`, `get_<entity>`, etc.)

3. **Add CLI commands** (`src/commands/`)
   - [ ] Create command module
   - [ ] Register in `main.rs`

4. **Update tests**
   - [ ] Add entity to `test_compact_preserves_all_entity_types` in `cli_maintenance_test.rs`
   - [ ] Add unit tests for the new entity

5. **Update documentation**
   - [ ] Add to PRD.md if appropriate
   - [ ] Update README.md

### Why This Matters

The `rebuild_cache()` function reconstructs the SQLite cache from JSONL files. If a new entity type is not added to this function:
- The cache will be missing data after `bn compact` runs
- Entity lookups will fail mysteriously
- Data appears "lost" even though it exists in JSONL files

The `test_compact_preserves_all_entity_types` test exists specifically to catch this bug early.
