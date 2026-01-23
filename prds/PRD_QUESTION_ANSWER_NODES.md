# PRD: Question/Answer Nodes

## Overview

Introduce two new entity types: **Question** (`bnq-xxxx`) and **Answer** (`bna-xxxx`) that enable agents and humans to capture clarifying questions about any resource and link answers to them. This creates a knowledge capture mechanism that preserves the context and reasoning behind decisions.

## Problem Statement

Currently, when working with binnacle:

1. **Context loss**: Agents often need clarification about tasks, PRDs, or bugs, but there's no structured way to capture these questions
2. **Scattered information**: Q&A happens in chat sessions, commit messages, or external docs—never linked to the relevant entity
3. **Repeated questions**: Different agents ask the same clarifying questions because answers aren't persisted
4. **Decision archaeology**: Understanding "why was this done this way?" requires reading through commit history or asking the original author
5. **Blocked work**: Agents may guess at ambiguous requirements instead of explicitly flagging uncertainty

## Motivation

With Question/Answer nodes, agents and humans can:
- Flag uncertainty explicitly by creating a question linked to a task/PRD/bug
- Preserve decision context by linking answers that explain the "why"
- Avoid repeated clarification by querying existing Q&A pairs
- Surface unanswered questions as blockers that need human input
- Build institutional knowledge that persists across sessions

---

## Proposed Solution

### Entity Type: Question

| Property | Type | Description |
|----------|------|-------------|
| `id` | string | Format: `bnQ-xxxx` (uppercase Q to distinguish from queue) |
| `type` | string | Always `"question"` |
| `title` | string | Brief question summary |
| `description` | string | Full question with context |
| `status` | enum | `open` \| `answered` \| `stale` \| `closed` |
| `priority` | int | 0-4, indicates urgency (0 = blocking, 4 = nice-to-know) |
| `author` | string? | Who asked the question |
| `tags` | string[] | Categorization (e.g., `requirements`, `architecture`, `scope`) |
| `created_at` | datetime | When asked |
| `updated_at` | datetime | Last modified |
| `answered_at` | datetime? | When first answer was linked |

### Entity Type: Answer

| Property | Type | Description |
|----------|------|-------------|
| `id` | string | Format: `bna-xxxx` |
| `type` | string | Always `"answer"` |
| `content` | string | The answer text |
| `author` | string? | Who provided the answer |
| `accepted` | bool | Whether this is the accepted answer |
| `question_id` | string | ID of the question being answered |
| `created_at` | datetime | When answered |
| `updated_at` | datetime | Last modified |

### Status Flow

**Question:**
```
open → answered → closed
         ↘ stale (auto-detects outdated answers)
```

- **open**: Awaiting an answer
- **answered**: Has at least one linked answer
- **stale**: Answer may be outdated (linked entity changed significantly)
- **closed**: Resolved, no longer relevant

**Answer:**
- Answers don't have status—they exist or don't
- One answer per question can be marked `accepted`

---

## Feature 1: Basic Question CRUD

### Commands

```bash
bn question create "Title" [-d "description"] [-p N] [-t tag] [-a author]
bn question list [--status S] [--tag T] [--priority N] [--open] [--unanswered]
bn question show bnQ-xxxx
bn question update bnQ-xxxx [--title|--description|--status|--priority|...]
bn question close bnQ-xxxx [--reason "..."]
bn question delete bnQ-xxxx

# Shortcuts
bn q create "Title"          # Short alias
bn q list --open             # Open questions only
```

### Example Usage

```bash
$ bn question create "Should themes support dark mode by default?" -p 1 -t requirements
{"id":"bnQ-a1b2","type":"question","title":"Should themes support dark mode by default?","status":"open"}

$ bn question list -H
3 question(s):

⚪ bnQ-a1b2 [open] P1 Should themes support dark mode by default? [requirements]
✅ bnQ-c3d4 [answered] P2 How should errors be displayed? [ux]
⚪ bnQ-e5f6 [open] P0 What authentication method to use? [security]
```

---

## Feature 2: Answer Management

### Commands

```bash
bn answer create bnQ-xxxx "Answer content" [-a author]
bn answer list bnQ-xxxx
bn answer show bna-xxxx
bn answer update bna-xxxx --content "Updated answer"
bn answer accept bna-xxxx              # Mark as accepted answer
bn answer delete bna-xxxx

# Shortcut
bn a create bnQ-a1b2 "Yes, dark mode should be the default"
```

### Answer Linking

When an answer is created:
1. Answer is automatically linked to the question via `question_id`
2. Question status changes to `answered` (if first answer)
3. Log entry records the answer with author

### Example

```bash
$ bn answer create bnQ-a1b2 "Yes, dark mode should be the default based on user research showing 73% prefer dark themes" -a henry
{"id":"bna-f7g8","question_id":"bnQ-a1b2","accepted":false}

$ bn answer accept bna-f7g8
Answer bna-f7g8 marked as accepted

$ bn question show bnQ-a1b2 -H
Question bnQ-a1b2 [answered]
  Title: Should themes support dark mode by default?
  Priority: 1
  Tags: requirements
  Asked by: agent-claude (2026-01-23T10:00:00Z)

  Description:
    The PRD doesn't specify whether dark mode should be opt-in or default.
    Need clarification before implementing theme switching logic.

  Answers:
    ✓ bna-f7g8 [accepted] by henry (2026-01-23T10:30:00Z)
      "Yes, dark mode should be the default based on user research
       showing 73% prefer dark themes"
```

---

## Feature 3: Linking Questions to Entities

Questions can be linked to any binnacle entity to provide context:

### Link Commands

```bash
# Link question to a task
bn link add bnQ-xxxx bn-task --type questions

# Link question to a PRD
bn link add bnQ-xxxx bn-prd --type questions

# Link question to a bug
bn link add bnQ-xxxx bn-bug --type questions

# Convenience: create question already linked
bn question create "Title" --about bn-a1b2
```

### Viewing Linked Questions

```bash
$ bn task show bn-a1b2 -H
Task bn-a1b2 [in_progress]
  Title: Implement theme configuration
  ...
  
  Questions:
    ⚪ bnQ-a1b2 [open] Should themes support dark mode by default?
    ✅ bnQ-c3d4 [answered] How should theme files be structured?

$ bn link list bn-a1b2 --type questions
Questions about bn-a1b2:
  ⚪ bnQ-a1b2 [open] Should themes support dark mode by default?
  ✅ bnQ-c3d4 [answered] How should theme files be structured?
```

---

## Feature 4: Unanswered Questions as Soft Blockers

### Integration with `bn ready`

Open questions with priority 0-1 on a task create a "soft blocker" state:

```bash
$ bn ready -H
Ready tasks (with warnings):

⚠️ bn-a1b2 [pending] Implement theme configuration
   Has 1 unanswered P0/P1 question(s)

✓ bn-c3d4 [pending] Add theme picker UI
✓ bn-e5f6 [pending] Write theme documentation
```

### Integration with `bn orient`

```bash
$ bn orient -H
Binnacle - AI agent task tracker

Current State:
  Total tasks: 42
  Ready: 3
  Blocked: 2
  In progress: 1
  
  ⚠️ 2 unanswered questions need attention:
    bnQ-a1b2 [P0] What authentication method to use?
    bnQ-e5f6 [P1] Should themes support dark mode?
```

### Question Priority Levels

| Priority | Meaning | Effect on `bn ready` |
|----------|---------|----------------------|
| 0 | Blocking | Task shown with ⚠️ warning |
| 1 | Important | Task shown with ⚠️ warning |
| 2 | Normal | Task shown normally |
| 3 | Low | Task shown normally |
| 4 | Nice-to-know | Task shown normally |

---

## Feature 5: Stale Answer Detection

When a linked entity is significantly modified, answers may become stale:

### Staleness Triggers

1. Task description changes by >50% (Levenshtein distance)
2. PRD content is updated after answer was written
3. Manual mark: `bn question stale bnQ-xxxx`

### Staleness Display

```bash
$ bn question list --stale -H
1 stale question(s):

⚠️ bnQ-c3d4 [stale] How should theme files be structured?
   Answer from 2026-01-15, task updated 2026-01-22
   Consider re-reviewing the answer
```

### Auto-Notification

When a question becomes stale:
1. Status changes to `stale`
2. Log entry records the triggering change
3. `bn orient` mentions stale questions

---

## Feature 6: Question Search and Discovery

### Search Commands

```bash
# Find questions about a topic
bn question search "dark mode"

# Find unanswered questions
bn question list --unanswered

# Find questions by entity
bn question list --about bn-a1b2

# Find questions I asked
bn question list --author agent-claude
```

### Knowledge Base Query

```bash
$ bn question search "authentication" -H
Questions matching "authentication":

✅ bnQ-x1y2 [answered] What authentication method to use?
   → bna-z3w4: "Use JWT with refresh tokens per RFC 7519"
   Linked to: bn-auth-task

⚪ bnQ-m5n6 [open] Should we support OAuth providers?
   Linked to: bn-auth-prd
```

---

## GUI Integration

### Graph View

Question nodes displayed with:
- **Shape**: Diamond (distinct from task circles, PRD rectangles)
- **Color**: 
  - Orange for `open` (needs attention)
  - Green for `answered`
  - Yellow for `stale`
  - Gray for `closed`
- **Size**: Smaller than tasks
- **Edge**: Dashed line to linked entity with `?` label

Answer nodes:
- **Shape**: Small circle attached to question
- **Color**: Green if accepted, gray otherwise
- **Position**: Clustered near their question

### Info Panel

When a question is selected:
- Full question text
- List of answers with accept/reject buttons
- Quick action to add answer
- Link to related entity

### Filter

Add filter toggle: "Show Questions" to hide/show Q&A nodes in the graph.

---

## MCP Integration

### New Tools

| Tool | Description |
|------|-------------|
| `bn_question_create` | Create a new question |
| `bn_question_list` | List questions with filtering |
| `bn_question_show` | Show question with answers |
| `bn_question_update` | Update question fields |
| `bn_question_close` | Close a question |
| `bn_answer_create` | Add an answer to a question |
| `bn_answer_accept` | Mark an answer as accepted |
| `bn_answer_list` | List answers for a question |

### New Resources

- `binnacle://questions` - All questions (subscribable)
- `binnacle://questions/open` - Open questions only

### New Prompts

| Prompt | Description |
|--------|-------------|
| `ask_clarification` | Help formulate a clear question about a task/PRD |
| `answer_question` | Help draft an answer to a question |
| `review_questions` | Review and triage open questions |

---

## Data Model

### Question Schema (JSON)

```json
{
  "id": "bnQ-a1b2",
  "type": "question",
  "title": "Should themes support dark mode by default?",
  "description": "The PRD doesn't specify whether dark mode should be opt-in or default. Need clarification before implementing theme switching logic.",
  "status": "answered",
  "priority": 1,
  "author": "agent-claude",
  "tags": ["requirements", "ux"],
  "created_at": "2026-01-23T10:00:00Z",
  "updated_at": "2026-01-23T10:30:00Z",
  "answered_at": "2026-01-23T10:30:00Z"
}
```

### Answer Schema (JSON)

```json
{
  "id": "bna-f7g8",
  "type": "answer",
  "content": "Yes, dark mode should be the default based on user research showing 73% prefer dark themes",
  "author": "henry",
  "accepted": true,
  "question_id": "bnQ-a1b2",
  "created_at": "2026-01-23T10:30:00Z",
  "updated_at": "2026-01-23T10:30:00Z"
}
```

### Link Types

| Link Type | Source | Target | Meaning |
|-----------|--------|--------|---------|
| `questions` | Question | Any Entity | Question is about the entity |

### Storage

```
~/.local/share/binnacle/<repo-hash>/
├── tasks.jsonl
├── questions.jsonl    # NEW
├── answers.jsonl      # NEW
├── commits.jsonl
├── test-results.jsonl
├── cache.db
└── config.toml
```

---

## Testing Strategy

### Unit Tests

- Question model serialization round-trip
- Answer model serialization round-trip
- Status transitions (open → answered → closed)
- Stale detection logic
- Priority validation (0-4)
- Question-answer linking

### Integration Tests

- Full Question CRUD round-trip
- Full Answer CRUD round-trip
- Create question linked to task
- Answer creation updates question status
- Accept answer marks as accepted
- `bn ready` shows warning for unanswered P0/P1
- `bn orient` shows unanswered questions
- `bn question search` finds relevant questions
- Stale detection triggers on entity update

### Test Count Estimate

- 12 unit tests (models, status, linking)
- 18 integration tests (CRUD, linking, ready integration, search)
- **Total new tests: ~30**

---

## Implementation Plan

### Phase 1: Core Question Entity
- [ ] Add Question model to `src/models/`
- [ ] Add Question storage to JSONL backend
- [ ] Implement basic CRUD commands (`bn question create/list/show/update/close/delete`)
- [ ] Add `bn q` alias

### Phase 2: Answer Entity
- [ ] Add Answer model to `src/models/`
- [ ] Add Answer storage to JSONL backend
- [ ] Implement answer commands (`bn answer create/list/show/update/accept/delete`)
- [ ] Add `bn a` alias
- [ ] Auto-update question status on first answer

### Phase 3: Entity Linking
- [ ] Add `questions` link type
- [ ] Implement `--about` flag for `bn question create`
- [ ] Show linked questions in `bn task/prd/bug show`

### Phase 4: Workflow Integration
- [ ] Add unanswered question warnings to `bn ready`
- [ ] Add unanswered questions section to `bn orient`
- [ ] Implement `bn question search`
- [ ] Add `--unanswered` and `--about` filters

### Phase 5: Stale Detection
- [ ] Implement staleness triggers
- [ ] Auto-mark questions as stale on entity changes
- [ ] Add `--stale` filter to list command

### Phase 6: MCP Tools
- [ ] Add all `bn_question_*` tools
- [ ] Add all `bn_answer_*` tools
- [ ] Add `binnacle://questions` resource
- [ ] Add prompts: `ask_clarification`, `answer_question`, `review_questions`

### Phase 7: GUI Integration
- [ ] Add Question node rendering (diamond shape)
- [ ] Add Answer node rendering (small circles)
- [ ] Update info panel for question selection
- [ ] Add "Show Questions" filter toggle

---

## Success Criteria

1. Questions can be created and linked to any binnacle entity in <5 seconds
2. Answers are linked to questions and display cleanly
3. `bn ready` warns about tasks with unanswered high-priority questions
4. `bn orient` surfaces unanswered questions that need attention
5. Questions can be searched to find existing clarifications
6. Stale answers are detected when linked entities change
7. GUI displays Q&A nodes distinctly and allows quick answer creation
8. Full traceability: Entity ← Question ← Answer chain is queryable

---

## Open Questions

1. Should questions support voting/upvoting to surface popular questions?
2. Should there be a notification system when questions are answered?
3. Should questions auto-close after N days if low priority?
4. Should answers support markdown formatting?
5. Should we allow multiple accepted answers (for different aspects)?

---

## Appendix: Command Reference

```bash
# Questions
bn question create "Title" [-d "desc"] [-p N] [-t tag] [-a author] [--about <entity-id>]
bn question list [--status S] [--tag T] [--priority N] [--open] [--unanswered] [--about <id>] [--stale]
bn question show <id>
bn question update <id> [--title|--description|--status|--priority|...]
bn question close <id> [--reason "..."]
bn question search "query"
bn question delete <id>
bn q <subcommand>                    # Alias

# Answers
bn answer create <question-id> "content" [-a author]
bn answer list <question-id>
bn answer show <id>
bn answer update <id> --content "..."
bn answer accept <id>
bn answer delete <id>
bn a <subcommand>                    # Alias

# Linking
bn link add <question-id> <entity-id> --type questions
bn link list <entity-id> --type questions
```
