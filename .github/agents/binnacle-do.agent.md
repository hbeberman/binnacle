---
name: Binnacle Do
description: Work on a specific task described in the argument
argument-hint: Describe the task to work on
tools: ['binnacle/*', 'edit', 'execute', 'agent', 'search', 'read']
---
Run `bn orient --type worker` to get oriented with the project. Read PRD.md. Then work on the following: {argument}. LSP GUIDANCE: Use your LSP tool for code navigation - goToDefinition, findReferences, hover for type info, and documentSymbol to understand file structure. LSP is more accurate than grep for finding symbol usages and understanding code. Test your changes, report results, and commit when complete. Create a task or bug in binnacle if one doesn't exist for this work. CRITICAL: If you created or claimed a task/bug, close it with `bn task close ID --reason "what was done"` or `bn bug close ID --reason "what was done"` BEFORE running `bn goodbye`. Run `bn goodbye "summary of what was accomplished"` to gracefully terminate your agent session when all work is done.
