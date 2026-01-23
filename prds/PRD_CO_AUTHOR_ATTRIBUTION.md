# PRD: Co-Author Attribution for Agent Commits

## Overview

Automatically attribute commits made during agent sessions to the binnacle-bot GitHub account using Git's `Co-authored-by` trailer. This provides visible attribution on GitHub, showing the binnacle-bot avatar alongside commits where an AI agent contributed.

## Problem Statement

When AI agents make commits on behalf of users, there's no visible indication that an agent was involved. This makes it difficult to:
1. Identify which commits had agent involvement when reviewing history
2. Track agent contribution patterns across repositories
3. Give proper attribution to the tooling that assisted

## Solution

Implement a `commit-msg` Git hook that:
1. Detects when a commit is made during an active agent session
2. Automatically appends a `Co-authored-by` trailer to the commit message
3. Links to the binnacle-bot GitHub account for visible attribution

## Target Users

- Developers using binnacle with AI agents (via `agent.sh` or direct `bn orient`)
- Teams who want visibility into agent-assisted commits
- Project maintainers reviewing contribution history

## Success Criteria

1. Commits made during agent sessions show binnacle-bot's avatar on GitHub
2. Feature is enabled by default but can be disabled via config
3. Hook detection is reliable (no false positives/negatives)
4. Zero overhead for non-agent commits

## Design

### Co-Author Trailer Format

```
Co-authored-by: binnacle-bot <noreply@binnacle.bot>
```

**GitHub Requirements**: The email `noreply@binnacle.bot` must be verified on the binnacle-bot GitHub account (https://github.com/binnacle-bot) for the avatar to display.

### Agent Session Detection

The `commit-msg` hook determines agent activity by checking:

1. **bn orient was called**: Session state file exists with valid timestamp
2. **Active agent PID**: The committing process (or its parent chain) matches the registered agent PID

Session state location:
```
~/.local/share/binnacle/<repo-hash>/session.json
```

Session state schema:
```json
{
  "agent_pid": 12345,
  "agent_type": "worker",
  "started_at": "2026-01-23T23:00:00Z",
  "orient_called": true
}
```

### Hook Behavior

```bash
# Pseudocode for commit-msg hook
if bn_config_get("co-author.enabled") == "false":
    exit 0

session = read_session_state()
if session is None or not session.orient_called:
    exit 0

if not is_agent_process(session.agent_pid):
    exit 0

# Agent is active - append trailer
if "Co-authored-by: binnacle-bot" not in commit_message:
    append_trailer(commit_message, "Co-authored-by: binnacle-bot <noreply@binnacle.bot>")
```

### Configuration

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `co-author.enabled` | bool | `true` | Enable/disable automatic co-author attribution |
| `co-author.name` | string | `binnacle-bot` | Name in Co-authored-by trailer |
| `co-author.email` | string | `noreply@binnacle.bot` | Email in Co-authored-by trailer |

Usage:
```bash
bn config set co-author.enabled false    # Disable feature
bn config set co-author.name "my-bot"    # Custom name
bn config set co-author.email "bot@example.com"  # Custom email
```

### Hook Installation

The hook is installed during `bn init` or `bn system init`:

```bash
bn init          # Installs hook to .git/hooks/commit-msg
bn system init   # Same, with other system setup
```

**Existing hook handling**:
- If `.git/hooks/commit-msg` exists, append binnacle logic (with guard to prevent duplication)
- Include clear comments marking binnacle's section
- Provide `bn hooks uninstall` to remove binnacle hooks

## Implementation Tasks

| ID | Task | Priority | Dependencies |
|----|------|----------|--------------|
| bn-918a | Track agent session state for hook detection | P1 | - |
| bn-c892 | Add co-author.enabled config option | P1 | - |
| bn-3a0a | Implement commit-msg hook for Co-authored-by trailer | P1 | bn-918a, bn-c892 |
| bn-291a | Install commit-msg hook during bn init/system init | P2 | bn-3a0a |
| bn-2ea3 | Add configurable co-author identity | P3 | - |

## Non-Goals

- Modifying existing commits (only affects new commits)
- Supporting multiple co-authors per agent session
- Integration with other Git hosting platforms (GitLab, Bitbucket) - may work but not tested

## Testing Strategy

### Unit Tests
- Session state serialization/deserialization
- PID ancestry detection logic
- Config option parsing

### Integration Tests
- Hook appends trailer when agent active
- Hook does nothing when no agent active
- Hook respects `co-author.enabled = false`
- Hook doesn't duplicate trailer on amend
- Installation preserves existing hooks

### Manual Verification
- Push commit to GitHub and verify binnacle-bot avatar appears
- Verify trailer format matches GitHub's expected format

## Rollout

1. Implement session tracking (bn-918a)
2. Add config option (bn-c892)  
3. Implement hook logic (bn-3a0a)
4. Add hook installation to init (bn-291a)
5. Add configurable identity (bn-2ea3)
6. Document in README

## Future Considerations

- Per-agent attribution (different co-authors for different agent types)
- Opt-in for human sessions (manual `bn session start`)
- Statistics on agent contribution percentage
- Integration with `bn log` to show agent involvement
