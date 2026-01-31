<!-- BEGIN BINNACLE SECTION -->
# Agent Instructions

This project uses **bn** (binnacle) for long-horizon task/test status tracking. Run `bn orient` to get started!

**After running `bn orient`**, report your assigned `agent_id` (e.g., `bn-486c`) to the user. This ID identifies your session in binnacle's tracking system.

For new projects, the human should run `bn session init` (for repo-specific setup) or `bn system init` (for first-time global setup). If you absolutely must initialize without human intervention, use `bn orient --init` (uses conservative defaults, skips optional setup).

### System vs Session Commands

Binnacle has two administrative namespaces:
- **`bn system`** - Host-global operations (stored in `~/.config/binnacle/`)
  - `bn system init` - First-time global setup (run once per machine)
  - `bn system copilot` - Copilot binary management
  - `bn system emit` - Emit embedded templates
  - `bn system build-info` - Build metadata
  - `bn system sessions` - List all known repos on this host
- **`bn session`** - Repo-specific operations (stored in `~/.local/share/binnacle/<REPO_HASH>/`)
  - `bn session init` - Initialize binnacle for this repository
  - `bn session store` - Import/export/inspect data
  - `bn session migrate` - Migrate storage backends
  - `bn session hooks` - Git hooks management

**Note**: The old commands (`bn system store`, `bn system migrate`, `bn system hooks`) still work but are deprecated. They print warnings directing you to the `bn session` equivalents.

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
- **NEVER run `git config user.email` or `git config user.name`** - Git identity is provided by the host. If git complains about missing identity, report the error - do not attempt to fix it.
- Commit early and often with clear messages
- Always run `just check` before committing

The task graph drives development priorities. Always update task status to keep it accurate.

**Tip**: Use `bn show <id>` to view any entity by ID - it auto-detects the type from the prefix (bn-, bnt-, bnq-).

## Finding Context for Your Task

When starting work on a task, you often need to understand:
- **Why** it exists (the parent PRD or milestone)
- **What related work** is happening (sibling tasks)
- **What subtasks** depend on it (child tasks)

Use these graph navigation commands to explore the task hierarchy:

### `bn graph lineage <id>`
Walk **up** the ancestry chain to find the PRD, milestone, or parent task that explains why this task exists.

```bash
# Find the PRD or milestone that spawned this task
bn graph lineage bn-xxxx

# Limit to 5 hops up the chain
bn graph lineage bn-xxxx --depth 5

# Include descriptions for more context
bn graph lineage bn-xxxx --verbose
```

**Use this when:** You need to understand the bigger picture or find documentation for your task.

### `bn graph peers <id>`
Find **sibling** tasks (tasks with the same parent) or **cousins** (tasks sharing a grandparent).

```bash
# Find sibling tasks (same parent)
bn graph peers bn-xxxx

# Find siblings and cousins (depth=2)
bn graph peers bn-xxxx --depth 2

# Include closed tasks in results
bn graph peers bn-xxxx --include-closed
```

**Use this when:** You want to see what other work is happening in parallel, or find similar completed tasks to reference.

### `bn graph descendants <id>`
Walk **down** to find child tasks and subtasks.

```bash
# Find immediate children and grandchildren (depth=3)
bn graph descendants bn-xxxx

# Find all descendants regardless of depth
bn graph descendants bn-xxxx --all

# Include closed/completed subtasks
bn graph descendants bn-xxxx --include-closed

# Limit to direct children only
bn graph descendants bn-xxxx --depth 1
```

**Use this when:** You're working on a high-level task and need to see what subtasks exist, or verify that all child work is complete.

### Quick Reference

| Goal | Command |
|------|---------|
| Find the PRD or parent goal | `bn graph lineage <id>` |
| Find related parallel work | `bn graph peers <id>` |
| Find subtasks or child work | `bn graph descendants <id>` |
| See all connected entities | `bn show <id>` (includes edges) |

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

### ⚠️ CRITICAL: System bn vs Development Build

**This project has TWO different `bn` binaries - confusing them will cause you to test the wrong code!**

#### 1. System bn (`~/.local/bin/bn`)
The **installed version** used for task tracking and cluster communication:
- ✅ Use for: `bn orient`, `bn ready`, `bn task update`, `bn goodbye`
- ✅ This is how you manage YOUR work on this project
- ❌ **DO NOT** test your code changes with this binary!

**PATH Note**: If `which bn` shows a different path (e.g., `/usr/local/sbin/bn`), you may be running an outdated version. Verify with `bn --version`. The `agent.sh` script automatically prioritizes `~/.local/bin/bn`.

#### 2. Development build (`./target/debug/bn` or `./target/release/bn`)
The **code you're actively developing** - what you're building/testing:
- ✅ Use `cargo run --` to test your bn code changes
- ✅ Use `cargo test` to run the test suite
- ✅ Use `just install` to promote dev build → system bn (after testing!)
- ❌ **DO NOT** use plain `bn` commands to test code changes

**Quick reference:**

| Purpose | Command | Which Binary |
|---------|---------|--------------|
| Task tracking | `bn orient`, `bn ready`, `bn task update` | System bn |
| Testing changes | `cargo run -- --help`, `cargo run -- task list` | Development build |
| Running tests | `cargo test`, `just test` | Development build |
| Install changes | `just install` | Copies dev → system |

**Example workflow:**
```bash
# 1. Make code changes to bn
# 2. Test your changes with development build
cargo run -- task list
cargo run -- --help

# 3. Run test suite
cargo test --all-features

# 4. If tests pass and you want to use your changes for task tracking
just install

# 5. Continue using system bn for task management
bn task close bn-xxxx --reason "completed"
```

### GUI Testing

1. When testing gui use "just gui" to launch it

## GUI Testing Best Practices

When testing GUI changes, follow this workflow to avoid disrupting the user's session:

1. **Use `just gui`** - Builds with `--features gui`, installs to ~/.local/bin, and launches on port 3030
   - Each repo uses its own binary location, so running `just gui` won't kill GUI sessions from other repos
   - Within the same repo, it will restart the existing GUI (this is intended behavior)
2. **Prefer `just install` for iterative changes** - Rebuilds without launching a new instance
   - If the user has a GUI open, they can refresh to pick up changes
   - This avoids disrupting their browser session
3. **Different port** - Use `BN_GUI_PORT=3031 just gui` for a separate instance on a different port
4. **DON'T kill the user's GUI session** - If unsure whether the user has a GUI open, use `just install` instead of `just gui`

## GUI Development Workflow

When working on GUI features (frontend JavaScript/CSS changes):

1. **Use `just dev-gui`** for development:
   ```bash
   just dev-gui
   ```
   - Runs in development mode (`--dev` flag)
   - Serves assets directly from `web/` directory (no bundling)
   - Edit JS/CSS files and refresh browser to see changes instantly
   - Faster iteration than rebuilding the bundle

2. **Validate before committing**:
   ```bash
   just gui-check
   ```
   - Validates GUI loads without console errors using Lightpanda headless browser
   - CI will fail PRs that introduce console errors or warnings
   - Always run this before committing GUI changes
   - **Note**: If you need to run Lightpanda manually (not via just gui-check), always disable telemetry:
     ```bash
     LIGHTPANDA_DISABLE_TELEMETRY=true lightpanda <command>
     ```

3. **Test with bundled assets before committing**:
   ```bash
   # Build with bundle (production mode)
   cargo build --features gui
   
   # Or use just install to test the production build
   just install
   bn gui serve
   ```

4. **Bundle is cached automatically**:
   - `build.rs` hashes the `web/` directory
   - Only rebuilds bundle when files change
   - Saves time on repeated builds

**Architecture:**
- **Development mode** (`--dev`): Serves from filesystem, instant updates
- **Production mode** (default): Serves embedded bundle compressed in binary
- Bundle created by `scripts/bundle-web.sh` during `cargo build --features gui`

## Using the Work Queue

This repo has a work queue for prioritizing tasks. Queued tasks appear first in `bn ready`, non-queued tasks appear in "OTHER".

- `bn queue show` - See queued tasks
- `bn queue add <task-id>` - Add task to queue (prioritize it)
- `bn queue rm <task-id>` - Remove from queue
