<!-- BEGIN BINNACLE SECTION -->
# Agent Instructions

This project uses **bn** (binnacle) for long-horizon task/test status tracking. Run `bn orient` to get started!

**After running `bn orient`**, report your assigned `agent_id` (e.g., `bna-486c`) to the user. This ID identifies your session in binnacle's tracking system.

For new projects, the human should run `bn system init` which provides helpful prompts for setup.
If you absolutely must initialize without human intervention, use `bn orient --init` (uses conservative defaults, skips optional setup).

## Task Workflow (CRITICAL - READ CAREFULLY)

**⚠️ SINGLE TASK PER SESSION**: You must work on exactly ONE task or bug per session. After completing it, call `bn goodbye` and terminate. Another agent will handle the next task.

### The Complete Workflow:

1. **CLAIM ONE item**: Run `bn ready`, pick ONE task/bug, claim it with `bn task update <id> --status in_progress` (or `bn bug update`).
2. **WORK on that item**: Implement, test, and commit your changes.
3. **CLOSE the item**: Run `bn task close <id> --reason "what was done"` (or `bn bug close`).
4. **TERMINATE immediately**: Run `bn goodbye "summary"` and end your session.

### Why Single-Task Sessions Matter:
- **Focused work**: One task gets full attention and proper completion
- **Clean handoffs**: Each task has a clear owner and outcome
- **Better tracking**: Task status accurately reflects work state
- **Reduced errors**: No context-switching between unrelated work

### What NOT to Do:
- ❌ Pick multiple tasks from `bn ready`
- ❌ Start a second task after closing the first
- ❌ Continue working after calling `bn goodbye`
- ❌ Skip the goodbye call

### Additional Commands:
- **If blocked**: Run `bn task update <id> --status blocked`, then `bn goodbye`
- **For bugs**: Use `bn bug create/update/close` - not `bn task create --tag bug`
- **For ideas**: Use `bn idea create/list/show` - ideas are low-stakes seeds that can be promoted to tasks later

## Git Rules (CRITICAL)

- **NEVER run `git push`** - The human operator handles all pushes. Your job is to commit locally.
- Commit early and often with clear messages
- Always run `just check` before committing

The task graph drives development priorities. Always update task status to keep it accurate.

**Tip**: Use `bn show <id>` to view any entity by ID - it auto-detects the type from the prefix (bn-, bnt-, bnq-).

## Creating Tasks (Best Practices)

- **Always use short names** (`-s`): They appear in the GUI and make tasks scannable
  - `bn task create -s "short name" -d "description" "Full task title"`
- **Add dependencies with reasons**: `bn link add <task> <blocker> -t depends_on --reason "why"`
- **Link to milestones**: `bn link add <task> <milestone> -t child_of`

## Documentation Nodes (IMPORTANT)

Use **doc nodes** instead of creating loose markdown files. Doc nodes are tracked in the task graph and linked to relevant entities.

### When to Use Doc Nodes vs Markdown Files

**Use doc nodes for:**
- PRDs, specifications, and design documents
- Implementation notes that explain *why* something was built a certain way
- Handoff notes between agent sessions
- Any documentation that relates to specific tasks, bugs, or features

**Keep as regular files:**
- README.md, CONTRIBUTING.md, LICENSE (repo-level standard files)
- AGENTS.md (agent instructions - this file)
- Code documentation (doc comments, inline comments)

### Doc Node Commands

```bash
# Create a doc linked to a task
bn doc create bn-task -T "Implementation Notes" -c "Content here..."

# Create from a file
bn doc create bn-task -T "PRD: Feature" --file spec.md --type prd

# List, show, attach to more entities
bn doc list
bn doc show bn-xxxx
bn doc attach bn-xxxx bn-other-task

# Update (creates new version, preserves history)
bn doc update bn-xxxx -c "Updated content..."
```

### Doc Types

- `note` (default) - General documentation, notes
- `prd` - Product requirements documents
- `handoff` - Session handoff notes for the next agent

## Before Ending Your Session (IMPORTANT)

1. **Verify your ONE task is complete**: Tests pass, code is formatted, changes are committed
2. **Close your task**: `bn task close <id> --reason "what was done"`
3. **Terminate**: `bn goodbye "summary"` - then STOP working

⚠️ Do NOT start another task. Let another agent handle it.

## Workflow Stages

For complex features, suggest the human use specialized agents:

1. **@binnacle-plan** - Research and outline (for ambiguous or large tasks)
2. **@binnacle-prd** - Detailed specification (when plan is approved)
3. **@binnacle-tasks** - Create bn tasks from PRD
4. **Execute** - Implement with task tracking (you're here)

If a task seems too large or unclear, suggest the human invoke the planning workflow.

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

## Responding to CI Failures (CRITICAL)

When the human reports that CI has failed, you must:

1. **Ask to see the failure output** - Request the relevant error logs from the CI run
2. **Parse the error type** - CI runs these checks (in this order of likelihood):
   - **Format errors**: `cargo fmt --all -- --check` - Fix with `cargo fmt`
   - **Clippy warnings**: `cargo clippy --all-targets --all-features -- -D warnings` - Fix the specific lint
   - **Test failures**: `cargo test --all-features` - Fix the failing test or code
   - **Security issues**: `zizmor` (GitHub Actions security) - Fix workflow file issues

3. **Fix the issue locally** - Make the minimal change to address the error
4. **Verify the fix** - Run `just check` and `cargo test --all-features`
5. **Commit the fix** - Use a clear message like "fix: resolve clippy warning in X"

### Common CI Error Patterns

**Format failure:**
```
Diff in /path/to/file.rs at line N:
```
→ Run `cargo fmt` and commit

**Clippy failure:**
```
error: [lint-name]
  --> src/file.rs:line:col
```
→ Fix the specific lint, don't suppress with `#[allow(...)]` unless justified

**Test failure:**
```
---- test_name stdout ----
thread 'test_name' panicked at ...
```
→ Fix the test or the code it's testing

### Important

- **Never push** - Even to fix CI. The human handles all pushes.
- **Don't guess** - If you can't see the error, ask for it
- **One fix at a time** - Don't bundle unrelated changes with CI fixes

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
