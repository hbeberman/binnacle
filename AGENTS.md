<!-- BEGIN BINNACLE SECTION -->
# Agent Instructions

This project uses **bn** (binnacle) for long-horizon task/test status tracking. Run `bn orient` to get started!

## Task Workflow (IMPORTANT)

1. **Before starting work**: Run `bn ready` to see available tasks, then `bn task update <id> --status in_progress`
2. **After completing work**: Run `bn task close <id> --reason "brief description"`
3. **If blocked**: Run `bn task update <id> --status blocked`
4. **For bugs**: Use `bn bug create/update/close` - not `bn task create --tag bug`

The task graph drives development priorities. Always update task status to keep it accurate.

## CI Validation Requirements (CRITICAL)

Before committing ANY code changes:

1. **Format check**: Run `cargo fmt --check` - code MUST be properly formatted
2. **Lint check**: Run `cargo clippy --all-targets --all-features -- -D warnings` - NO warnings allowed
3. **Quick validation**: Run `just check` to run both format and clippy checks
4. **Tests**: Run `cargo test --all-features` if you modified code

**Pre-commit hook**: Run `git config core.hooksPath hooks` to enable the pre-commit hook that automatically validates formatting and linting before allowing commits.

**NEVER commit code that fails these checks.** CI will reject it and waste time.

## Before you mark task done (IMPORTANT)

1. Run `bn ready` to check if any related tasks should also be closed
2. Close ALL tasks you completed, not just the one you started with
3. Verify the task graph is accurate before finalizing your work
<!-- END BINNACLE SECTION -->

## Build and Test

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
