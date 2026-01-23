<!-- BEGIN BINNACLE SECTION -->
# Agent Instructions

This project uses **bn** (binnacle) for long-horizon task/test status tracking. Run `bn orient` to get started!

For new projects, the human should run `bn system init` which provides helpful prompts for setup.
If you absolutely must initialize without human intervention, use `bn orient --init` (uses conservative defaults, skips optional setup).

## Task Workflow (IMPORTANT)

1. **Before starting work**: Run `bn ready` to see available tasks, then `bn task update <id> --status in_progress`
2. **After completing work**: Run `bn task close <id> --reason "brief description"`
3. **If blocked**: Run `bn task update <id> --status blocked`
4. **When terminating**: Run `bn goodbye "summary of what was accomplished"` to gracefully end your session
5. **For bugs**: Use `bn bug create/update/close` - not `bn task create --tag bug`

The task graph drives development priorities. Always update task status to keep it accurate.

## Before you mark task done (IMPORTANT)

1. Run `bn ready` to check if any related tasks should also be closed
2. Close ALL tasks you completed, not just the one you started with
3. Verify the task graph is accurate before finalizing your work

Run `bn --help` for the complete command reference.
<!-- END BINNACLE SECTION -->

## CI Validation Requirements (CRITICAL)

Before committing ANY code changes:

1. **Format check**: Run `cargo fmt --check` - code MUST be properly formatted
2. **Lint check**: Run `cargo clippy --all-targets --all-features -- -D warnings` - NO warnings allowed
3. **Quick validation**: Run `just check` to run both format and clippy checks
4. **Tests**: Run `cargo test --all-features` if you modified code

**Pre-commit hook**: Run `git config core.hooksPath hooks` to enable the pre-commit hook that automatically validates formatting and linting before allowing commits.

**Pre-push hook**: The hooks directory also contains a pre-push hook that validates git tag versions don't exceed Cargo.toml version. This prevents accidentally pushing a tag that doesn't match the crate version.

**NEVER commit code that fails these checks.** CI will reject it and waste time.

## Build and Test

### IMPORTANT: System bn vs Development Build

This project has TWO different `bn` binaries you need to distinguish:

1. **System bn** (`~/.local/bin/bn`) - The installed version used to track tasks
   - Use plain `bn` commands for task management: `bn orient`, `bn task list`, etc.
   - This is what you use to manage YOUR work on this project

2. **Development build** (`./target/debug/bn` or `./target/release/bn`) - What you're building/testing
   - Use `cargo run --` to test the code you're developing
   - Use `just test` or `cargo test` to run the test suite
   - Use `just install` to install your changes to the system bn

**Quick reference:**

- Task tracking: `bn orient`, `bn task list`, `bn ready` (uses system bn)
- Testing code changes: `cargo run -- --help`, `cargo test` (uses dev build)
- Install your changes: `just install` (copies dev build → system bn)

### GUI Testing

1. When testing gui use "just gui" to launch it

## GUI Testing Best Practices

When testing GUI changes, follow this workflow to avoid excessive port approval requests:

1. **Use `just gui`** - This builds with `--features gui`, installs to ~/.local/bin, and launches on a consistent port (3030)
2. **Check if GUI is already running** - `just gui` will warn you if port 3030 is in use. The user may already have a GUI open in their browser
3. **For iterative changes** - After making code changes:
   - Run `just install` to rebuild and install (doesn't launch a new instance)
   - The user can refresh their browser to pick up changes (if the binary wasn't replaced while running)
   - Or stop the existing GUI and run `just gui` again
4. **Different port** - Use `BN_GUI_PORT=3031 just gui` if you need a separate instance

## Using the Work Queue

This repo has a work queue for prioritizing tasks. Queued tasks appear first in `bn ready`, non-queued tasks appear in "OTHER".

- `bn queue show` - See queued tasks
- `bn queue add <task-id>` - Add task to queue (prioritize it)
- `bn queue rm <task-id>` - Remove from queue
