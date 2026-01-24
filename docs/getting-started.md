# Getting Started with Binnacle

This guide walks you through setting up binnacle in a new project and using AI agents to build features with task tracking.

## Prerequisites

- Rust toolchain (for building from source)
- Git
- A GitHub Copilot subscription (for agent automation)

## Installation

```bash
# Clone and install binnacle
git clone https://github.com/hbeberman/binnacle.git
cd binnacle
cargo install --path . --features gui

# Verify installation
bn --version
```

The binary installs to `~/.cargo/bin/bn`. Make sure this is in your PATH.

## Tutorial: Build a Tic-Tac-Toe Game with Agent Assistance

This tutorial demonstrates binnacle's agent workflow by creating a simple web-based tic-tac-toe game.

### Step 1: Create a New Project

```bash
# Create project directory
mkdir tictactoe && cd tictactoe

# Initialize git
git init

# Create basic structure
mkdir -p src
echo "node_modules/" > .gitignore
git add .gitignore && git commit -m "Initial commit"
```

### Step 2: Initialize Binnacle

```bash
# Interactive setup (recommended for first-time users)
bn system init
```

This will:
- Create the binnacle data store for your repo
- Set up AGENTS.md with instructions for AI agents
- Optionally configure hooks and other settings

You can verify the setup:

```bash
bn orient -H
```

### Step 3: Plan the Feature

Create tasks for the tic-tac-toe implementation:

```bash
# Create a milestone for the feature
bn milestone create "Tic-Tac-Toe MVP" -s "ttt-mvp"

# Create tasks with short names for easy scanning
bn task create "Set up HTML/CSS game board" -s "game board" -p 1 --tag frontend
bn task create "Implement game state logic" -s "game logic" -p 1 --tag core
bn task create "Add win detection" -s "win detection" -p 2 --tag core
bn task create "Add player turn indicator" -s "turn indicator" -p 2 --tag frontend
bn task create "Add reset button" -s "reset button" -p 3 --tag frontend

# Link tasks to milestone (assuming bnm-xxxx is your milestone ID)
# Run `bn milestone list` to get the actual ID
bn link add bn-xxxx bnm-yyyy --type child_of
```

Add dependencies to model the build order:

```bash
# Win detection depends on game logic
bn link add bn-<win-detection> bn-<game-logic> --type depends_on --reason "needs game state"

# Turn indicator depends on game logic
bn link add bn-<turn-indicator> bn-<game-logic> --type depends_on --reason "needs current player"
```

### Step 4: View Ready Tasks

```bash
bn ready -H
```

This shows tasks with no blockers, sorted by priority. You'll see tasks like "game board" and "game logic" are ready since they have no dependencies.

### Step 5: Run an Agent

With binnacle's agent launcher, you can let an AI agent work through your tasks:

```bash
# From the binnacle repo directory, run an agent on your project
cd /path/to/binnacle
./agent.sh auto
```

The agent will:
1. Run `bn orient` to understand the project state
2. Run `bn ready` to find available tasks
3. Claim a task with `bn task update <id> --status in_progress`
4. Implement the feature
5. Commit the changes
6. Close the task with `bn task close <id> --reason "..."`
7. Gracefully exit with `bn goodbye`

### Step 6: Monitor Progress

While the agent works (or afterwards), you can monitor progress:

```bash
# See current state
bn -H

# Check what's blocked and why
bn blocked -H

# View milestone progress
bn milestone show bnm-xxxx -H

# Open the GUI for visual tracking
bn gui
# Then open http://localhost:3030 in your browser
```

### Step 7: Loop Mode for Continuous Work

For multiple tasks, use loop mode:

```bash
./agent.sh --loop auto
```

This restarts the agent after each task completion, working through your backlog. Press `Ctrl+C` twice to exit.

## Key Concepts

### Task States

Tasks flow through these states:

```
pending → in_progress → done
                     ↘ blocked
                     ↘ cancelled
```

### Priority Levels

- `0` - Critical (work on immediately)
- `1` - High
- `2` - Medium (default)
- `3` - Low
- `4` - Nice to have

### Dependencies

Use `bn link add` to model task relationships:

```bash
# Task A depends on Task B (B must complete first)
bn link add bn-A bn-B --type depends_on --reason "needs API first"
```

The `bn ready` command only shows tasks whose dependencies are satisfied.

### Work Queue

Prioritize specific tasks for agents:

```bash
# Create a queue
bn queue create "Sprint 1"

# Add high-priority tasks
bn queue add bn-xxxx

# Agents see queued tasks first in `bn ready`
```

## Tips for Agent-Assisted Development

1. **Write clear task titles** - Agents use these to understand what to build
2. **Add descriptions** - Use `-d "..."` for complex tasks
3. **Model dependencies** - Helps agents work in the right order
4. **Use short names** - `-s "short"` makes tasks easy to scan in the GUI
5. **Review agent commits** - Agents commit locally; you decide when to push

## Next Steps

- Read the [README](../README.md) for full command reference
- Explore the [PRD](../PRD.md) for design philosophy
- Check [CONTRIBUTING.md](../CONTRIBUTING.md) to contribute

## Troubleshooting

### "No ready tasks" but I have pending tasks

Check if they're blocked:
```bash
bn blocked -H
```

### Agent not finding tasks

Make sure binnacle is initialized:
```bash
bn orient
```

### GUI not starting

Ensure you installed with the GUI feature:
```bash
cargo install --path . --features gui
```
