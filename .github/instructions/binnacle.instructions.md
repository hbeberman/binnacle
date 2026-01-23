---
applyTo: '**'
---
# Binnacle Project Instructions

This project uses **binnacle** (`bn`) for task and workflow tracking.

## Quick Reference

- `bn orient` - Get project overview and current state
- `bn ready` - Show tasks ready to work on
- `bn task update <id> --status in_progress` - Start a task
- `bn task close <id> --reason "description"` - Complete a task
- `bn goodbye` - End your session gracefully

## Workflow Stages

For complex features, suggest the human use specialized agents:

1. **@binnacle-plan** - Research and outline (for ambiguous or large tasks)
2. **@binnacle-prd** - Detailed specification (when plan is approved)
3. **@binnacle-tasks** - Create bn tasks from PRD
4. **Execute** - Implement with task tracking (you're here)

If a task seems too large or unclear, suggest the human invoke the planning workflow.

Always update task status to keep the graph accurate.
