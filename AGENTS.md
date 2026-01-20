# Agent Instructions
This project uses **bn** (binnacle) for long-horizon task/test status tracking. Run `bn orient` to get started!

## Task Workflow (IMPORTANT)
1. **Before starting work**: Run `bn ready` to see available tasks, then `bn task update <id> --status in_progress`
2. **After completing work**: Run `bn task close <id> --reason "brief description"` 
3. **If blocked**: Run `bn task update <id> --status blocked`

The task graph drives development priorities. Always update task status to keep it accurate.

## Before you mark task done (IMPORTANT)
1. Run `bn ready` to check if any related tasks should also be closed
2. Close ALL tasks you completed, not just the one you started with
3. Verify the task graph is accurate before finalizing your work
