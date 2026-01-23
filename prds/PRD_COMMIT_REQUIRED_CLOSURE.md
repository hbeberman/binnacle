# PRD: Require Git Commit for Task Closure

**Status:** Implemented
**Author:** Claude
**Date:** 2026-01-22

## Overview

Add a configurable enforcement mechanism that prevents tasks from being closed as `done` unless they have at least one valid git commit linked to them. This ensures accountability and traceability by requiring proof of work before task completion.

## Motivation

1. **Accountability**: Tasks should have tangible evidence of work completion
2. **Traceability**: Git history should be linkable back to task tracking for audits
3. **Process enforcement**: Prevents accidental task closure without actual work
4. **Regression tracking**: Linked commits enable better regression detection when tests fail

## Non-Goals

- Automatic commit linking from commit messages (future work)
- Commit message format enforcement (out of scope)
- Requiring commits for `cancelled` status (intentionally excluded)
- Validating that the commit content relates to the task (too complex)

---

## Feature Specification

### Configuration

A new configuration option controls this behavior:

```bash
bn config set require_commit_for_close true   # Enable enforcement
bn config set require_commit_for_close false  # Disable (default)
```

**Default**: `false` (opt-in behavior to avoid breaking existing workflows)

### Behavior When Enabled

When `require_commit_for_close` is `true`:

1. `bn task close <id>` checks if the task has at least one linked commit
2. If no commits are linked → **error** with helpful message
3. If commits exist → closure proceeds normally
4. `bn task close <id> --force` bypasses the check (escape hatch)

### Commit Validation

The linked commit must:
- Exist in the git repository (validated via `git cat-file -t <sha>`)
- Be a valid commit object (not a tree or blob)

Invalid or missing commits are reported as warnings but don't block closure (the link exists, git history may have been rewritten).

### Affected Commands

| Command | Behavior with `require_commit_for_close=true` |
|---------|----------------------------------------------|
| `bn task close <id>` | Requires linked commit or `--force` |
| `bn task close <id> --force` | Bypasses commit requirement |
| `bn task update <id> --status done` | Requires linked commit or `--force` |
| `bn task update <id> --status cancelled` | No commit required |
| `bn task update <id> --status blocked` | No commit required |

### Error Messages

**No commits linked:**
```
Error: Cannot close task bn-a1b2 - no commits linked

This repository requires commits to be linked before closing tasks.
Link a commit with: bn commit link <sha> bn-a1b2
Or bypass with: bn task close bn-a1b2 --force

Hint: Run 'git log --oneline -5' to see recent commits.
```

**Commit not found in repo (warning only):**
```
Warning: Linked commit abc1234 not found in repository (may have been rebased)
Task bn-a1b2 closed.
```

### CLI Changes

#### `bn task close`

```
Close a task

Usage: bn task close [OPTIONS] <TASK_ID>

Arguments:
  <TASK_ID>  Task ID to close (e.g., bn-a1b2)

Options:
      --reason <REASON>  Reason for closing the task
      --force            Close even if no commits are linked (bypasses require_commit_for_close)
  -H, --human            Output in human-readable format instead of JSON
  -h, --help             Print help
```

#### `bn task update`

Add `--force` flag when updating status to `done`:

```
Update a task

Usage: bn task update [OPTIONS] <TASK_ID>

Arguments:
  <TASK_ID>  Task ID to update

Options:
      --status <STATUS>  New status [possible values: pending, in_progress, blocked, done, cancelled]
      --force            When setting status to done, bypass commit requirement
      ...existing options...
```

---

## Implementation Details

### Config Schema Addition

```rust
// In config.rs or similar
pub struct BinnacleConfig {
    // ... existing fields ...

    /// Require at least one linked commit before closing a task as done.
    /// Default: false
    pub require_commit_for_close: bool,
}
```

### Validation Flow

```
task_close(task_id, force) {
    task = load_task(task_id)

    if config.require_commit_for_close && !force {
        commits = get_commits_for_task(task_id)
        if commits.is_empty() {
            return Error::CommitRequired(task_id)
        }

        // Optional: validate commits exist in repo
        for commit in commits {
            if !git_commit_exists(commit.sha) {
                warn!("Commit {} not found in repository", commit.sha)
            }
        }
    }

    // Proceed with closure
    task.status = Status::Done
    task.closed_at = now()
    save_task(task)
}
```

### Git Validation

```rust
fn git_commit_exists(sha: &str) -> bool {
    Command::new("git")
        .args(["cat-file", "-t", sha])
        .output()
        .map(|o| o.status.success() && String::from_utf8_lossy(&o.stdout).trim() == "commit")
        .unwrap_or(false)
}
```

---

## User Workflows

### Enabling the Feature

```bash
# Enable commit requirement
bn config set require_commit_for_close true

# Verify setting
bn config get require_commit_for_close
```

### Normal Workflow (with feature enabled)

```bash
# Work on a task
bn task update bn-a1b2 --status in_progress

# Make commits
git commit -m "Implement feature X"

# Link the commit
bn commit link abc1234 bn-a1b2

# Now closure works
bn task close bn-a1b2 --reason "Feature complete"
```

### Bypass When Necessary

```bash
# Documentation-only task with no code changes
bn task close bn-doc1 --force --reason "Updated README only"
```

### Checking Before Close

```bash
# See if commits are linked
bn commit list bn-a1b2

# If empty, find recent commits
git log --oneline -10

# Link relevant commit
bn commit link def5678 bn-a1b2
```

---

## Implementation Tasks

| ID | Task | Priority | Dependencies |
|----|------|----------|--------------|
| bn-6b41 | Add `require_commit_for_close` config option | P1 | - |
| bn-f923 | Implement commit count check in task close | P1 | bn-6b41 |
| bn-69f3 | Add `--force` flag to `bn task close` | P1 | bn-f923 |
| bn-c6f0 | Add `--force` flag to `bn task update --status done` | P1 | bn-f923 |
| bn-c721 | Implement git commit existence validation | P2 | bn-f923 |
| bn-6ca7 | Add helpful error messages with hints | P1 | bn-f923 |
| bn-9251 | Update CLI help text | P1 | bn-69f3, bn-c6f0 |
| bn-ee36 | Add unit tests for commit requirement logic | P1 | bn-f923 |
| bn-8032 | Add integration tests for feature | P1 | bn-69f3, bn-c6f0, bn-6ca7 |
| bn-b2db | Update documentation | P2 | bn-69f3, bn-c6f0 |

---

## Testing Strategy

### Unit Tests

- Config: `require_commit_for_close` defaults to `false`
- Config: Can set/get `require_commit_for_close`
- Close: With config=false, closure works without commits
- Close: With config=true and no commits, returns error
- Close: With config=true and commits linked, succeeds
- Close: With `--force`, bypasses requirement
- Update: `--status done` respects same rules as close
- Update: `--status cancelled` ignores commit requirement
- Validation: Git commit existence check works

### Integration Tests

```bash
# Test: Cannot close without commit when enabled
bn config set require_commit_for_close true
bn task create "Test task"
bn task close bn-xxxx  # Should fail
bn commit link $(git rev-parse HEAD) bn-xxxx
bn task close bn-xxxx  # Should succeed

# Test: Force bypasses requirement
bn task create "Another task"
bn task close bn-yyyy --force  # Should succeed

# Test: Disabled by default
bn config set require_commit_for_close false
bn task create "Third task"
bn task close bn-zzzz  # Should succeed
```

---

## Future Considerations

1. **Auto-linking from commit messages**: Parse `bn-xxxx` from commit messages
2. **Commit count requirements**: Require minimum N commits
3. **Branch-based linking**: Auto-link all commits from a feature branch
4. **Per-task override**: Allow individual tasks to opt-out
5. **Audit report**: Show tasks closed with `--force` for review

---

## Decisions Made

1. **Opt-in by default**: Setting defaults to `false` to avoid breaking existing workflows
2. **Force flag**: Provides escape hatch for legitimate cases (docs, config changes)
3. **Cancelled exempt**: Only `done` status requires commits; `cancelled` does not
4. **Warning for missing commits**: Invalid commit refs warn but don't block (git rebase scenario)
5. **Validation is optional**: Commit existence check is best-effort, not blocking

---

## Implementation Notes

All features described in this PRD have been implemented:
- `bn config set/get require_commit_for_close` works
- `bn task close` and `bn task update --status done` enforce commit requirement when enabled
- `--force` flag bypasses the requirement
- Error messages include helpful hints about linking commits
- Unit and integration tests verify the behavior
