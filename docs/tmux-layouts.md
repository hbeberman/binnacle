# Tmux Layout Files

Binnacle supports declarative tmux session management through KDL layout files. These files define windows, panes, and optional startup commands for reproducible development environments.

## Quick Start

Create a layout file at `.binnacle/tmux/dev.kdl`:

```kdl
layout "my-project" {
    window "editor" {
        pane dir="." {
            cmd "git status"
        }
    }
}
```

Load it with:

```bash
bn session tmux load dev --project
```

## Layout File Locations

Layouts are discovered in priority order:

| Source | Path | Use Case |
|--------|------|----------|
| Project | `.binnacle/tmux/*.kdl` | Project-specific layouts (commit to repo) |
| Session | `~/.local/share/binnacle/<hash>/tmux/*.kdl` | Per-project user layouts |
| User | `~/.config/binnacle/tmux/*.kdl` | Global personal layouts |

When the same layout name exists in multiple locations, the first match wins.

## KDL Schema Reference

### `layout` Node

The root node defining a tmux session:

```kdl
layout "session-name" {
    window "window-name" { ... }
}
```

| Attribute | Required | Description |
|-----------|----------|-------------|
| name (positional) | Yes | Session name for tmux |

### `window` Node

Defines a tmux window containing one or more panes:

```kdl
window "window-name" {
    pane { ... }
}
```

| Attribute | Required | Description |
|-----------|----------|-------------|
| name (positional) | Yes | Window title |

### `pane` Node

Defines a pane within a window:

```kdl
pane split="horizontal" size="70%" dir="/path" {
    cmd "command"
}
```

| Attribute | Required | Description | Example |
|-----------|----------|-------------|---------|
| `split` | No | Split direction from parent | `"horizontal"`, `"vertical"`, `"h"`, `"v"` |
| `size` | No | Pane size | `"70%"` (percentage) or `"20"` (lines) |
| `dir` | No | Working directory | `"."`, `"~"`, `"/absolute/path"` |

### `cmd` / `command` Node

Specifies a command to run when the pane is created:

```kdl
pane {
    cmd "git status"
}
```

Both `cmd` and `command` are accepted as node names.

## The `cmd` Attribute: Intended Use

The `cmd` attribute is designed for **short commands that produce output and exit**, such as:

- `git status` - Display repository status
- `bn` - Show binnacle task overview
- `ls -la` - List directory contents  
- `cat README.md` - Display file contents
- `cargo test --list` - List available tests

### Shell Persistence

When a command finishes, the pane stays open with an interactive shell. Internally, commands are wrapped with:

```bash
your-command; exec ${SHELL:-/bin/sh}
```

This means you can run a quick command and still have a working shell afterward.

### Long-Running Interactive Tools

For long-running interactive tools, **launch them directly in the terminal** rather than using the `cmd` attribute:

| Tool | Recommendation |
|------|----------------|
| `htop`, `btop` | Launch manually after session loads |
| `watch` | Launch manually |
| `vim`, `nvim` | Launch manually |
| `cargo watch` | Launch manually |

**Why avoid `cmd` for these tools?**

1. The shell-persistence wrapping is unnecessary for tools that run indefinitely
2. When the tool eventually exits (e.g., you quit vim), the shell exec may produce unexpected behavior
3. These tools are inherently interactive and don't benefit from automated startup

### Example: Mixed Layout

```kdl
layout "dev" {
    window "main" {
        // Editor pane - launch your editor manually here
        pane dir="./src"
        
        // Status pane - shows output and keeps shell
        pane split="horizontal" size="30%" {
            cmd "git status && bn"
        }
    }
    
    window "monitoring" {
        // Empty pane for you to start htop/btop
        pane dir="."
        
        // Log viewer pane
        pane split="vertical" {
            cmd "tail -20 ~/.local/share/binnacle/*/actions.log"
        }
    }
}
```

## Directory Paths

The `dir` attribute supports several path formats:

| Format | Description | Example |
|--------|-------------|---------|
| Absolute | Used as-is | `"/home/user/project"` |
| Tilde | Expands to home | `"~/repos/myproject"` |
| Relative | Relative to cwd when loading | `"."`, `"./src"`, `"../sibling"` |

## Complete Example

```kdl
layout "binnacle-dev" {
    window "code" {
        // Main editor pane (70% width)
        pane split="horizontal" size="70%" dir="./src"
        
        // Terminal pane (30% width)
        pane size="30%" dir="." {
            cmd "bn -H"
        }
    }
    
    window "test" {
        // Test output pane
        pane dir="." {
            cmd "cargo test --list 2>/dev/null | head -20"
        }
        
        // Manual test runner pane
        pane split="horizontal" dir="."
    }
    
    window "docs" {
        // Documentation browser
        pane dir="./docs"
    }
}
```

## Commands

```bash
# List available layouts
bn session tmux list

# Load a layout
bn session tmux load <name> [--project|--session|--user]

# Save current tmux session as a layout
bn session tmux save <name> [--project|--session|--user]

# Attach to a loaded session
tmux attach -t <session-name>
```

## Troubleshooting

### "can't find window" errors

This typically occurs when tmux server isn't running. Binnacle automatically runs `tmux start-server` first, but if you see this error, try:

```bash
tmux start-server
bn session tmux load <name>
```

### Panes close immediately

If using `cmd` with a command that exits quickly, the shell-persistence wrapper should keep the pane open. If panes still close, check:

1. The command syntax is valid
2. You're not using `exit` in your command chain

### Commands run but output not visible

There may be a race condition where `send-keys` executes before the shell is ready. Try re-running the layout:

```bash
tmux kill-session -t <session-name>
bn session tmux load <name>
```
