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

> ⚠️ **Important: Use a Separate Repository Clone**  
> When working with binnacle-tracked projects, it's recommended to use a separate clone of your repository for agent work. This isolates agent-driven development from your main working directory and prevents conflicts with uncommitted changes or active work-in-progress.
>
> If you're contributing to binnacle itself or tracking an existing repo with binnacle, consider maintaining separate clones:
> - One for your manual development work
> - One dedicated to agent-driven task execution

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

# Link tasks to milestone (assuming bn-yyyy is your milestone ID)
# Run `bn milestone list` to get the actual ID
bn link add bn-xxxx bn-yyyy --type child_of
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
# Run an auto worker agent (uses container mode by default)
bn-agent auto

# Or run on host directly
bn-agent --host auto
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
bn milestone show bn-xxxx -H

# Open the GUI for visual tracking
bn gui
# Then open http://localhost:3030 in your browser
```

### Step 7: Continuous Work (Loop Mode)

By default, `bn-agent` loops continuously, restarting after each task. Use `--once` to run a single iteration:

```bash
bn-agent auto           # loops by default (Ctrl+C twice to exit)
bn-agent --once auto    # run once and exit
```

This works through your backlog automatically. Press `Ctrl+C` twice to exit loop mode.

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
- See [Embedding the Viewer](./embedding-viewer.md) for sharing task graphs

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
