<p align="center">
  <img src="binnaclebanner.png" alt="Binnacle Banner" width="100%">
</p>

# binnacle

Task tracker for AI agents. Stores data outside your repo so it doesn't pollute your codebase.

> [!WARNING]
> Early alpha. Things may break.

## Install

```bash
cargo install --path .
```

## Usage

```bash
bn system init                  # set up in your project
bn task create "Do the thing"   # create a task
bn ready                        # see what's actionable
bn task close bn-xxxx           # mark done
```

For AI agents:
```bash
bn orient                       # get up to speed on project state
bn goodbye "summary"            # graceful exit
```

## Running Agents

```bash
./agent.sh auto                 # pick highest priority task and work on it
./agent.sh --loop auto          # keep going until queue is empty
./agent.sh buddy                # helper for adding tasks interactively
```

See [container/README.md](container/README.md) for sandboxed execution.

## What It Tracks

- **Tasks** (`bn-xxxx`) with priorities, dependencies, tags
- **Bugs** (`bnb-xxxx`) with severity levels
- **Ideas** (`bni-xxxx`) that can be promoted to tasks
- **Milestones** (`bnm-xxxx`) with progress tracking
- **Tests** (`bnt-xxxx`) linked to tasks, auto-reopen on regression
- **Queue** (`bnq-xxxx`) for agent prioritization

## Quick Reference

```bash
bn                              # status summary
bn ready                        # actionable tasks
bn blocked                      # what's waiting on dependencies
bn show <id>                    # details on any entity

bn task create/list/update/close
bn bug create/list/update/close
bn link add <src> <tgt> --type depends_on
bn queue show                   # see prioritized work

bn gui                          # web interface (needs --features gui)
bn mcp serve                    # MCP server for agents
```

Run `bn --help` for everything else.

## GUI

```bash
cargo install --path . --features gui
bn gui
# open http://localhost:3030
```

Interactive graph of tasks and dependencies with live updates.

## Building

```bash
just install                    # recommended, includes GUI
cargo build --release           # without GUI
```

## License

MIT
