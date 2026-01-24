# PRD: Doc Nodes

**Related ideas:** `bn-9cdf` (doc node concept), `bn-82de` (doc history exploration)

## Overview

Introduce a new entity type called **Doc** (`bn-xxxx`) for attaching markdown documentation to any entity in the graph. Docs serve as a primary communication channel between humans and agents, capturing context, decisions, PRDs, handoff notes, and other persistent knowledge.

## Problem Statement

Currently, binnacle entities have only titles and short descriptions. When working on complex features:
- Context gets lost between agent sessions
- Decisions aren't documented alongside the work they affect
- PRDs live as separate files with no graph connectivity
- Handoff notes have no standard home
- There's no audit trail of how documentation evolved

## Proposed Solution

### Entity Type: Doc

| Property | Type | Description |
|----------|------|-------------|
| `id` | string | Format: `bn-xxxx` (hash-based) |
| `type` | string | Always `"doc"` |
| `doc_type` | enum | `prd` \| `note` \| `handoff` |
| `title` | string | Document title |
| `content` | string | Markdown content (zstd + base64 encoded in JSONL) |
| `summary_dirty` | bool | True if content changed but #Summary section didn't |
| `editors` | Editor[] | List of who edited this version |
| `supersedes` | string? | ID of previous version (if this is an update) |
| `created_at` | datetime | When created |
| `updated_at` | datetime | Last modified |

### Editor Schema

```json
{
  "editor_type": "agent",  // "agent" | "user"
  "identifier": "bna-57f9" // agent ID or username
}
```

### Content Format

- **Markdown only** with expected `# Summary` section at the top
- **Storage**: zstd compressed, base64 encoded in JSONL; decompressed in SQLite cache
- **Size limit**: 5KB compressed+encoded maximum
- **Syntax highlighting**: syntect for CLI `--full` display

Example doc content:
```markdown
# Summary
Brief agent-provided summary of this document.

# Title
Full document content here...
```

## Feature 1: Doc CRUD

### Commands

```bash
bn doc create <entity-id> "Title" --type prd [options]   # Create linked to entity
bn doc show <doc-id>                                      # Show summary
bn doc show <doc-id> --full                               # Show full syntax-highlighted content
bn doc update <doc-id> [options]                          # Create new version
bn doc list [--type T] [--edited-by E] [--for <entity>]   # List docs
bn doc history <doc-id>                                   # Show version history
bn doc delete <doc-id>                                    # Delete doc
```

### Content Input Options

```bash
# Inline content
bn doc create bn-1234 "Design Doc" --type prd -c "# Summary\n..."

# From file
bn doc create bn-1234 "Design Doc" --type prd --file ./design.md

# From stdin
cat design.md | bn doc create bn-1234 "Design Doc" --type prd --stdin

# With agent-provided summary
bn doc create bn-1234 "Design Doc" --type prd --file ./design.md \
  --short "Explains the architecture for feature X"
```

### Key Design Decisions

1. **Must link on creation** - `bn doc create` requires at least one entity ID
2. **Summary as markdown section** - `# Summary` at top, not a separate field
3. **Dirty flag** - Tracks when content changes but summary doesn't (hash comparison excluding summary section)
4. **New entity per edit** - Each `bn doc update` creates a new entity with `supersedes` link

## Feature 2: Versioning System

### How Versioning Works

When `bn doc update <id>` is called:

1. Create new doc entity with new `bn-xxxx` ID
2. Copy all content/metadata with modifications
3. Set `supersedes: <old-id>` on new doc
4. **Transfer all edges** from old doc to new doc, EXCEPT:
   - Edges explicitly marked as `pinned` (allows pointing to specific versions)
5. Track editor in `editors` array

### Pinned Edges

Entities can deliberately point to older doc versions:

```bash
bn link add bn-task-123 bn-old-doc-456 --type related --pinned
```

When the doc is updated, this edge stays pointing to the old version.

### History Access

```bash
$ bn doc history bn-abc1 -H
Doc bn-abc1 "Architecture Overview"
3 versions:

  bn-abc1 (current) - 2026-01-24 by user:henry
  bn-9f3e           - 2026-01-23 by agent:bna-57f9
  bn-2d4a (original)- 2026-01-22 by user:henry

$ bn doc show bn-9f3e --full   # View specific version
```

### Dirty Summary Detection

- Hash content excluding `# Summary` section
- If content hash changes but summary hash doesn't → set `summary_dirty: true`
- When agent updates summary → automatically clears `summary_dirty`
- Manual clear: `bn doc update <id> --clear-dirty`

**Important**: User-facing editors should only commit doc updates with explicit user permission to avoid spamming version history with character-by-character changes.

## Feature 3: Linking & Discovery

### Link Type

Uses generic `related` link type (bidirectional):

```bash
# Both are equivalent
bn link add bn-doc-123 bn-task-456 --type related
bn link add bn-task-456 bn-doc-123 --type related
```

### Docs Linking to Docs

Docs can link to other docs (e.g., adding notes to a PRD):

```bash
bn doc create bn-prd-doc "Implementation Notes" --type note
bn link add bn-notes-doc bn-prd-doc --type related
```

### Discovery in `bn show`

When showing any entity, display linked doc summaries:

```bash
$ bn show bn-task-123 -H
Task bn-task-123 [in_progress]
  Title: Implement auth middleware
  ...

  📄 Related Docs (2):
    bn-abc1 [prd] "Auth Architecture" - Explains JWT validation approach
    bn-def2 [note] "Security Considerations" - Notes on token expiry

$ bn show bn-task-123 --full -H
# Shows full doc content for all linked docs
```

### Orphan Detection

`bn doctor` warns about orphaned docs (no linked entities):

```
⚠ Warning: Doc bn-xyz9 "Untitled" has no linked entities
```

### NOT in `bn orient`

Docs are not included in orient output to keep it focused on actionable items.

## Feature 4: Doc Types & Filtering

### Doc Types

| Type | Use Case |
|------|----------|
| `prd` | Product requirements, specifications |
| `note` | General notes, observations |
| `handoff` | Context for session handoffs when partial progress made |

### Filtering

```bash
bn doc list --type prd                    # All PRDs
bn doc list --edited-by agent:bna-57f9    # Docs edited by specific agent
bn doc list --edited-by user:henry        # Docs edited by user
bn doc list --for bn-task-123             # Docs linked to entity
bn doc list --type prd --for bn-milestone-1  # PRDs for a milestone
```

## Feature 5: GUI Integration (v1)

### New Docs Tab

- List view of all docs with type badges
- Filter by doc type, linked entity
- Click to view doc

### Doc Viewer

- Render markdown in webview
- Show `# Summary` section prominently
- Syntax highlighting for code blocks
- Show version history in sidebar
- Visual indicator for `summary_dirty` docs

### Graph View

- Doc nodes rendered distinctly (document icon?)
- `supersedes` edges shown as version chain
- `pinned` edges visually distinct

## MCP Integration

### New Tools

| Tool | Description |
|------|-------------|
| `bn_doc_create` | Create doc linked to entity |
| `bn_doc_show` | Show doc summary or full content |
| `bn_doc_update` | Create new version of doc |
| `bn_doc_list` | List docs with filters |
| `bn_doc_history` | Show version history |
| `bn_doc_delete` | Delete doc |

### New Resource

- `binnacle://docs` - All docs (subscribable)
- `binnacle://docs?type=prd` - Filtered by type
- `binnacle://docs?for=bn-xxxx` - Filtered by linked entity

### New Prompt

- `document_decision` - Create a note documenting a decision
- `create_handoff` - Create handoff context for session end
- `review_prd` - Review and annotate a PRD

## Data Model

### Doc Schema (JSON)

```json
{
  "id": "bn-abc1",
  "type": "doc",
  "doc_type": "prd",
  "title": "Auth Architecture",
  "content": "H4sIAAAAA...",  // zstd + base64
  "summary_dirty": false,
  "editors": [
    {"editor_type": "user", "identifier": "henry"},
    {"editor_type": "agent", "identifier": "bna-57f9"}
  ],
  "supersedes": "bn-9f3e",
  "created_at": "2026-01-24T10:00:00Z",
  "updated_at": "2026-01-24T14:30:00Z"
}
```

### Storage

Docs stored in `docs.jsonl`:

```
~/.local/share/binnacle/<repo-hash>/
├── tasks.jsonl
├── ideas.jsonl
├── docs.jsonl         # NEW
├── commits.jsonl
├── test-results.jsonl
├── cache.db           # Stores decompressed doc content
└── config.toml
```

### Link Schema (pinned edge)

```json
{
  "source": "bn-task-123",
  "target": "bn-doc-456",
  "link_type": "related",
  "pinned": true,
  "created_at": "2026-01-24T10:00:00Z"
}
```

## Testing Strategy

### Feature 1: Doc CRUD
- Unit tests: Doc model serialization, zstd compression round-trip, ID generation
- Integration tests: Create with inline/file/stdin, show summary vs full, list with filters

### Feature 2: Versioning
- Unit tests: Supersedes link creation, edge transfer logic, pinned edge preservation
- Integration tests: Update creates new version, history command, version-specific show

### Feature 3: Linking
- Unit tests: Bidirectional link resolution, orphan detection
- Integration tests: Show includes doc summaries, doctor warns on orphans

### Feature 4: Doc Types
- Unit tests: Type validation, filter logic
- Integration tests: Filter by type, edited-by, for-entity

### Feature 5: GUI
- Manual testing: Docs tab, markdown rendering, version history sidebar

## Implementation Notes

### Compression

```rust
use zstd::stream::{encode_all, decode_all};
use base64::{Engine, engine::general_purpose::STANDARD};

fn compress_content(content: &str) -> Result<String> {
    let compressed = encode_all(content.as_bytes(), 3)?;  // level 3
    Ok(STANDARD.encode(&compressed))
}

fn decompress_content(encoded: &str) -> Result<String> {
    let compressed = STANDARD.decode(encoded)?;
    let decompressed = decode_all(&compressed[..])?;
    Ok(String::from_utf8(decompressed)?)
}
```

### Summary Dirty Detection

```rust
fn is_summary_dirty(old_content: &str, new_content: &str) -> bool {
    let old_hash = hash_excluding_summary(old_content);
    let new_hash = hash_excluding_summary(new_content);
    let old_summary_hash = hash_summary_section(old_content);
    let new_summary_hash = hash_summary_section(new_content);
    
    // Content changed but summary didn't
    old_hash != new_hash && old_summary_hash == new_summary_hash
}
```

## Success Criteria

1. Docs can be created and linked in under 5 seconds
2. Version history is navigable and clear
3. `bn show` surfaces relevant docs without overwhelming output
4. Agents can create handoff docs when ending sessions with partial progress
5. PRDs are discoverable via `bn doc list --type prd`
6. GUI renders markdown beautifully with syntax highlighting

## Open Questions

1. ~~Should docs support other formats?~~ **Decided: Markdown only**
2. ~~Where to store content?~~ **Decided: zstd+base64 in JSONL, decompressed in cache**
3. ~~Link type?~~ **Decided: Generic `related` type**
4. Future: Doc templates (see idea `bn-3af0`)
5. Future: Full-text search across doc content (FTS5?)

## Related Work

- **Task `bn-7f7d`**: Fix agent assumption about three-letter ID prefixes
- **Idea `bn-3af0`**: Doc templates for common doc types
