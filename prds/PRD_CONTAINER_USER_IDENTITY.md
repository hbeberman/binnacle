# PRD: Container User Identity with nss_wrapper

**Status:** Implemented
**Author:** GitHub Copilot
**Date:** 2026-01-24
**Updated:** 2026-01-24

## Overview

Implement secure user identity handling for `bn container run` using nss_wrapper, allowing containers to run as the host user with proper identity for tools like Node.js, git, and other NSS-dependent applications.

## Motivation

The original container implementation had security issues:

- Made `/etc/passwd` and `/etc/shadow` world-writable to work around user mapping
- Used `--allow-new-privs` unconditionally, weakening container isolation
- Granted `ALL ALL=(ALL) NOPASSWD: ALL` sudo access

These hacks were meant to achieve:

1. Files created by agents have correct host ownership
2. Agents can run `sudo dnf install` for packages

**This PRD addresses goal #1.** Goal #2 (sudo/package installation) was deferred due to containerd 2.0 limitations — see "Deferred: --allow-sudo Mode" section below.

## Non-Goals

- Rootless containerd (running `ctr` without sudo) — tracked in bn-a3a2
- Sudo access inside containers — deferred, see below
- Network namespace isolation — requires separate work for AI API access

## Dependencies

- containerd with `--user UID:GID` support (already available)
- nss_wrapper package in container image

---

## Specification

### Operating Mode

Container runs as the host user directly via `--user UID:GID`, with nss_wrapper providing user identity.

| Aspect | Behavior |
|--------|----------|
| Container user | Host UID/GID |
| Sudo access | ❌ Not available |
| File ownership | ✅ Correct (native) |
| User identity | nss_wrapper for `os.userInfo()`, git |
| Security flags | None needed |
| Host requirements | None |

**Use case**: Agent work — reading/writing code, running builds, git operations, npm install (local packages).

### nss_wrapper for User Identity

The container runs as a UID that doesn't exist in `/etc/passwd`. This breaks:

- Node.js `os.userInfo()`
- Git operations that check user identity
- Any tool that calls `getpwuid()`

Solution: Use `nss_wrapper` with `LD_PRELOAD`:

1. **Containerfile**: Install `nss_wrapper`, create template `/etc/nss_wrapper/passwd`
2. **entrypoint.sh**: If not root, create user-local passwd copy and set env vars:

   ```bash
   export LD_PRELOAD=/usr/lib64/libnss_wrapper.so
   export NSS_WRAPPER_PASSWD="$HOME/.nss_wrapper/passwd"
   export NSS_WRAPPER_GROUP="$HOME/.nss_wrapper/group"
   ```

This intercepts NSS calls at userspace level without modifying system files.

---

## Implementation

### Files Modified

#### 1. `container/Containerfile`

```dockerfile
# Install nss_wrapper for user identity
RUN dnf install -y \
    # ... existing packages ...
    nss_wrapper \
    && dnf clean all

# Create nss_wrapper template files
RUN mkdir -p /etc/nss_wrapper && \
    cp /etc/passwd /etc/nss_wrapper/passwd && \
    cp /etc/group /etc/nss_wrapper/group && \
    chmod 644 /etc/nss_wrapper/passwd /etc/nss_wrapper/group

# REMOVED: chmod 666 /etc/passwd /etc/shadow
# REMOVED: ALL ALL=(ALL) NOPASSWD: ALL
```

#### 2. `container/entrypoint.sh`

```bash
#!/bin/bash
set -e

# Set up writable HOME
if [ ! -w "${HOME:-/}" ]; then
    export HOME="/tmp/agent-home"
fi
mkdir -p "$HOME"

# Set up user identity for non-root mode (nss_wrapper)
CURRENT_UID=$(id -u)
if [ "$CURRENT_UID" != "0" ]; then
    CURRENT_GID=$(id -g)

    NSS_WRAPPER_DIR="$HOME/.nss_wrapper"
    mkdir -p "$NSS_WRAPPER_DIR"

    cp /etc/nss_wrapper/passwd "$NSS_WRAPPER_DIR/passwd"
    cp /etc/nss_wrapper/group "$NSS_WRAPPER_DIR/group"

    echo "agent:x:${CURRENT_UID}:${CURRENT_GID}:Binnacle Agent:${HOME}:/bin/bash" >> "$NSS_WRAPPER_DIR/passwd"

    if ! grep -q ":${CURRENT_GID}:" "$NSS_WRAPPER_DIR/group"; then
        echo "agent:x:${CURRENT_GID}:" >> "$NSS_WRAPPER_DIR/group"
    fi

    export LD_PRELOAD=/usr/lib64/libnss_wrapper.so
    export NSS_WRAPPER_PASSWD="$NSS_WRAPPER_DIR/passwd"
    export NSS_WRAPPER_GROUP="$NSS_WRAPPER_DIR/group"

    echo "🔧 Running as user (UID $CURRENT_UID) - sudo not available"
fi

# ... rest of entrypoint ...
```

#### 3. `src/commands/mod.rs`

Container runs with `--user UID:GID` flag:

```rust
#[cfg(unix)]
{
    use std::os::unix::fs::MetadataExt;
    if let Ok(meta) = fs::metadata(&worktree_abs) {
        let uid = meta.uid();
        let gid = meta.gid();
        args.push("--user".to_string());
        args.push(format!("{}:{}", uid, gid));
    }
}
```

#### 4. `container/README.md`

Updated to document the security model.

---

## Testing

### Integration Tests

1. Verify container runs as host UID
2. Verify `whoami` returns "agent"
3. Verify files created in /workspace are owned by host user
4. Verify nss_wrapper environment is set up

### Manual Testing

```bash
bn container run ../test-worktree --shell
$ whoami        # Should show "agent"
$ id            # Should show host UID
$ env | grep NSS  # Should show nss_wrapper env vars
$ touch /workspace/test.txt
$ exit
ls -la ../test-worktree/test.txt  # Should be owned by you
```

---

## Deferred: --allow-sudo Mode

### Original Design

The original PRD included an `--allow-sudo` mode using user namespace mapping:

```
--uidmap 0:<HOST_UID>:1       # Container root → host user
--uidmap 1:<SUBUID_START>:<SUBUID_COUNT>  # Container 1+ → subuid range
```

This would allow running as root inside the container (enabling sudo/dnf) while preserving file ownership on the host.

### Why It Was Deferred

During implementation, we discovered that **containerd 2.0's user namespace support is incompatible with `--net-host`**:

1. User namespaces require mounting a new sysfs
2. `--net-host` shares the host network namespace
3. Mounting sysfs in a user namespace with shared network fails:

   ```
   mount sysfs: operation not permitted
   ```

Since binnacle containers require `--net-host` for AI agent API calls (OpenAI, Anthropic), user namespace mapping cannot currently be used.

### Future Work

Tracked in idea **bn-cab5**: Revisit `--allow-sudo` when a solution is found:

1. **slirp4netns**: User-mode networking that works with user namespaces
2. **Network proxy**: Route API calls through host-side proxy
3. **Future containerd**: May add support for this combination

### Removed Code

The subuid/subgid parsing code was implemented and tested but has been **removed** per YAGNI principles:

- `get_subid_range()`: Parsed `/etc/subuid` and `/etc/subgid`
- `subid_not_configured_error()`: Generated helpful setup instructions
- 5 unit tests covering various parsing scenarios

This code was removed since `--allow-sudo` has no timeline. It can be reimplemented from git history if needed.
