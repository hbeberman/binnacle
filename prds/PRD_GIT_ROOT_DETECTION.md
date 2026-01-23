# PRD: Git Root Storage Detection

**Status:** Implemented
**Author:** Claude
**Date:** 2026-01-22

## Overview

Automatically detect git repository root and use it as the canonical path for storage hashing. This prevents accidental creation of orphan databases when running binnacle from subdirectories. Git worktrees are resolved back to their main repository so all worktrees share the same binnacle database.

## Motivation

Running `bn` commands in a subdirectory creates a separate database:

```bash
cd ~/repos/myproject
bn orient  # Creates database hashed from ~/repos/myproject

cd ~/repos/myproject/src
bn orient  # Creates DIFFERENT database hashed from ~/repos/myproject/src
```

This leads to fragmented task tracking and user confusion.

## Non-Goals

- **Nested binnacle stores within a single git repo** (one repo = one store; use tags or git submodules instead)
- Parent directory scanning for non-git nested binnacle stores
- Storing repo_path metadata (git root detection is cheap at runtime)
- Interactive prompts for path selection (see PRD_ORIENT_INIT_FLAG.md)
- `--init` flag for explicit initialization (see PRD_ORIENT_INIT_FLAG.md)
- Test isolation via `BN_DATA_DIR` (see PRD_TEST_ISOLATION.md)

---

## Specification

### Git Root Detection Function

```rust
/// Find the git root by walking up directories looking for .git
/// For worktrees, resolves back to the main repository.
pub fn find_git_root(start: &Path) -> Option<PathBuf> {
    let mut current = start.canonicalize().ok()?;
    loop {
        let git_path = current.join(".git");
        if git_path.is_dir() {
            return Some(current);
        } else if git_path.is_file() {
            // Git worktree - resolve to main repo
            if let Some(main_repo) = resolve_worktree_to_main_repo(&git_path) {
                return Some(main_repo);
            }
            return Some(current); // Fallback
        }
        if !current.pop() {
            return None;
        }
    }
}
```

### Worktree Resolution

Git worktrees have a `.git` file containing `gitdir: /path/to/main/.git/worktrees/<name>`. Resolution:

1. Parse `gitdir:` from `.git` file
2. Read `commondir` file (points to main `.git`)
3. Return parent of main `.git` as repo root

### Integration Point

In `main.rs`, replace:

```rust
let repo_path = env::current_dir()?;
```

With:

```rust
let cwd = env::current_dir()?;
let repo_path = find_git_root(&cwd).unwrap_or(cwd);
```

### Behavior Matrix

| Scenario | Before | After |
|----------|--------|-------|
| Run from git root | Uses git root | Uses git root (same) |
| Run from git subdirectory | Uses subdirectory ❌ | Uses git root ✅ |
| Run from non-git directory | Uses CWD | Uses CWD (same) |
| Nested git repos | N/A | Uses innermost git root ✅ |
| Git worktree | Uses worktree path ❌ | Uses main repo root ✅ |
| Explicit `--repo` / `-C` flag | N/A | Uses specified path literally ✅ |

### Explicit Path Override

For cases where auto-detection doesn't work as expected, or for scripting/CI:

```bash
# Use -C flag (like git -C)
bn -C /path/to/repo task list

# Or use environment variable
BN_REPO=/path/to/repo bn task list
```

**Priority:** `--repo` flag > `BN_REPO` env > auto-detect git root > cwd

The explicit path:

- Must exist (fails early with clear error)
- Bypasses git root detection entirely (explicit is explicit)
- Useful for scripting, CI, and non-git directories

---

## Implementation

**Files modified:**

| File | Change |
|------|--------|
| `src/storage/mod.rs` | Add `find_git_root()`, `resolve_worktree_to_main_repo()`, integrate into `get_storage_dir()` |
| `src/cli/mod.rs` | Add `-C`/`--repo` global flag with `BN_REPO` env var |
| `src/main.rs` | Update path resolution with flag priority and existence check |

Total: ~100 lines of code.

---

## Testing

```rust
#[test]
fn test_find_git_root_from_subdir() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();

    // Create git repo structure
    std::fs::create_dir(root.join(".git")).unwrap();
    std::fs::create_dir_all(root.join("src/lib")).unwrap();

    let result = find_git_root(&root.join("src/lib"));
    assert_eq!(result, Some(root.to_path_buf()));
}

#[test]
fn test_find_git_root_not_in_repo() {
    let temp = tempfile::tempdir().unwrap();
    let result = find_git_root(temp.path());
    assert_eq!(result, None);
}

#[test]
fn test_find_git_root_nested_repos() {
    let temp = tempfile::tempdir().unwrap();
    let outer = temp.path();
    let inner = outer.join("nested");

    // Create nested git repos
    std::fs::create_dir(outer.join(".git")).unwrap();
    std::fs::create_dir_all(inner.join(".git")).unwrap();
    std::fs::create_dir_all(inner.join("src")).unwrap();

    // From inner/src, should find inner (most specific)
    let result = find_git_root(&inner.join("src"));
    assert_eq!(result, Some(inner.canonicalize().unwrap()));
}
```

---

## Migration

- **Existing users**: Databases created from subdirectories will become orphaned
- **New behavior**: All commands from any subdirectory use the git root's database
- **No breaking changes**: Users at git root see no difference
