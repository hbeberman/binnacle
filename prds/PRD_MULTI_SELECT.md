# PRD: Multi-Select and Batch Operations in GUI

**Status:** Draft  
**Priority:** Medium-High  

## Overview

Enable selection of multiple entities simultaneously in the binnacle GUI, unlocking batch operations and relationship-building workflows that would otherwise require tedious one-by-one CLI commands.

## Problem Statement

Currently, the GUI supports only single-entity selection:
- Users can click on one node at a time
- Creating relationships between entities requires CLI commands or navigating back and forth
- Batch operations (queue multiple tasks, close related bugs, etc.) are impossible in the GUI
- Comparing entities side-by-side isn't supported
- There's no way to visually "collect" a working set of related items

This friction discourages use of the GUI for actual work management, relegating it to a visualization-only tool.

## Proposed Solution

Implement a multi-select system with:
1. **Selection mechanics** - Shift+click, Ctrl+click, drag-select
2. **Visual feedback** - Clear indication of selected entities
3. **Batch actions panel** - Contextual toolbar when multiple items selected
4. **Relationship builder** - First-class support for linking selected entities

### Selection Mechanics

#### Click Modifiers
| Action | Behavior |
|--------|----------|
| Click | Select only this entity (clears others) |
| Ctrl/Cmd + Click | Toggle this entity in selection |
| Shift + Click | Range select (if applicable) or add to selection |
| Escape | Clear all selections |

#### Drag Selection (Box Select)
- Hold Shift + drag to draw a selection rectangle
- All entities within the rectangle get selected
- Works in addition to existing pan behavior (standard drag)

#### Keyboard Support
| Key | Behavior |
|-----|----------|
| Ctrl/Cmd + A | Select all visible entities (respects filters) |
| Escape | Clear selection |
| Delete/Backspace | Open confirmation for bulk delete (when allowed) |

### Visual Feedback

**Selected nodes:**
```
┌──────────────────┐
│  ████████████   │  ← Thicker glow border (not just highlight)
│  █  bn-a1b2  █   │  ← Semi-transparent selection overlay
│  ████████████   │
└──────────────────┘
```

**Selection counter badge:**
```
┌─────────────────────────────────────────┐
│  🔵 3 selected                          │  ← Floating badge, top-right of canvas
└─────────────────────────────────────────┘
```

**Multi-selection info panel:**
- When 2+ entities selected, info panel transforms into batch view
- Shows summary: "3 tasks, 1 bug selected"
- Lists selected entities with checkboxes for individual removal
- Displays batch action buttons

### Batch Actions

When multiple entities are selected, expose contextual actions:

#### Universal Actions (all entity types)
| Action | Description |
|--------|-------------|
| **Link Together** | Create relationships between all selected entities |
| **Add to Queue** | Queue all selected tasks/bugs |
| **Remove from Queue** | Unqueue all selected |
| **Summarize** | Feed selection context to AI agent for interactive summary |
| **Export** | Export selected entities to clipboard (markdown/JSON) |

#### Task/Bug Specific
| Action | Description |
|--------|-------------|
| **Close All** | Close all selected (with reason prompt) |
| **Block All** | Set status to blocked |
| **Assign Parent** | Set all selected as children of another entity |

#### Relationship Builder Mode

When exactly 2 entities are selected, show dedicated relationship panel:

```
┌────────────────────────────────────────────────────┐
│  Link: bn-a1b2 ←→ bn-c3d4                          │
├────────────────────────────────────────────────────┤
│  ○ bn-a1b2 depends_on bn-c3d4                      │
│  ○ bn-a1b2 blocks bn-c3d4                          │
│  ○ bn-a1b2 related_to bn-c3d4                      │
│  ○ bn-a1b2 child_of bn-c3d4                        │
│  ○ bn-a1b2 duplicate_of bn-c3d4                    │
├────────────────────────────────────────────────────┤
│  Reason: [optional text field________________]     │
├────────────────────────────────────────────────────┤
│  [← Swap Direction]  [Cancel]  [Create Link]       │
└────────────────────────────────────────────────────┘
```

**3+ entities selected → Chain linking:**
```
┌────────────────────────────────────────────────────┐
│  Link 4 entities                                   │
├────────────────────────────────────────────────────┤
│  Mode:                                             │
│  ○ Chain (A→B→C→D)  - Sequential dependencies      │
│  ○ Hub (all→selected one)  - One entity blocks all │
│  ○ Mesh (all↔all)  - Everything related            │
├────────────────────────────────────────────────────┤
│  Link type: [depends_on ▼]                         │
├────────────────────────────────────────────────────┤
│  [Cancel]  [Preview]  [Create Links]               │
└────────────────────────────────────────────────────┘
```

### Summarize Agent

The **Summarize** action opens an interactive chat interface with an AI agent that has full context of the selected entities.

**How it works:**
1. User selects multiple entities (tasks, bugs, docs, ideas, etc.)
2. Clicks "Summarize" in the batch actions panel
3. A chat modal opens with a summary agent
4. The agent has already ingested:
   - Full content of all selected entities (titles, descriptions, statuses)
   - Relationships between them
   - Recent activity/history
   - Related doc nodes if any
5. Agent provides an initial summary, then user can ask follow-up questions

**Example conversation:**
```
┌────────────────────────────────────────────────────────────────┐
│  📊 Summarizing 5 entities                              [×]    │
├────────────────────────────────────────────────────────────────┤
│                                                                │
│  🤖 Agent:                                                     │
│  You've selected 3 tasks and 2 bugs related to the            │
│  authentication system:                                        │
│                                                                │
│  **Tasks:**                                                    │
│  - bn-a1b2: Implement OAuth2 flow (in_progress)               │
│  - bn-c3d4: Add refresh token handling (pending, blocked by ↑)│
│  - bn-e5f6: Write auth integration tests (pending)            │
│                                                                │
│  **Bugs:**                                                     │
│  - bn-g7h8: Token expires prematurely (open, high priority)   │
│  - bn-i9j0: Login fails on Safari (open, medium priority)     │
│                                                                │
│  The token expiry bug (bn-g7h8) may be related to the OAuth2  │
│  implementation. Want me to suggest a dependency link?         │
│                                                                │
├────────────────────────────────────────────────────────────────┤
│  👤 You: What's the critical path here?                        │
│                                                                │
│  🤖 Agent:                                                     │
│  The critical path is:                                         │
│  1. bn-a1b2 (OAuth2 flow) - currently in progress              │
│  2. bn-g7h8 (token expiry bug) - likely needs OAuth2 done     │
│  3. bn-c3d4 (refresh tokens) - explicitly blocked by OAuth2   │
│  4. bn-e5f6 (tests) - should come after implementation        │
│                                                                │
│  The Safari bug (bn-i9j0) appears independent and could be    │
│  worked in parallel.                                           │
│                                                                │
├────────────────────────────────────────────────────────────────┤
│  [____________________________________] [Send]                 │
│                                                                │
│  Quick actions: [Create suggested links] [Export summary]      │
└────────────────────────────────────────────────────────────────┘
```

**Agent capabilities:**
- Summarize the selection's purpose and current state
- Identify patterns (common blockers, related work, gaps)
- Suggest missing relationships or dependencies
- Answer questions about the selected work
- Help draft status updates or standup notes
- Recommend prioritization based on dependencies
- Spot potential issues (circular deps, orphaned work)

**Integration with actions:**
- Agent can suggest actions: "Create link between X and Y?"
- User confirms → action executes directly from chat
- Keeps conversation context while taking actions

### Use Cases Unlocked

#### 1. Sprint Planning
Select multiple tasks → "Add to Queue" → Instantly prioritize a batch of work

#### 2. Bug Triage
Select related bugs → Link as `related_to` → Group connected issues

#### 3. Dependency Mapping
Select task chain → "Chain link" with `depends_on` → Define execution order

#### 4. Feature Grouping
Select feature tasks → Assign all as `child_of` milestone → Organize hierarchy

#### 5. Duplicate Detection
Select suspected duplicates → Mark one as `duplicate_of` another → Clean up graph

#### 6. Quick Export
Select relevant entities → Export as markdown → Paste into standup notes

#### 7. Bulk Cleanup
Select completed/stale items → Close all with shared reason → Fast housekeeping

#### 8. Context Summarization
Select related tasks/bugs/docs → "Summarize" → AI agent reads all context and starts interactive conversation about the selection

## Implementation

### Phase 1: Selection Infrastructure (~2-3 sessions)

**State changes (`state.js`):**
```javascript
// Replace:
selectedNode: null,

// With:
selectedNodes: [],  // Array of node IDs

// Add helper methods:
function isSelected(nodeId) { ... }
function toggleSelection(nodeId) { ... }
function selectRange(startId, endId) { ... }
function clearSelection() { ... }
function selectAll() { ... }
```

**Camera/input changes (`camera.js`):**
- Detect modifier keys (Ctrl, Shift) on click
- Implement box-select overlay when Shift+drag
- Update node selection based on modifiers

**Renderer changes (`renderer.js`):**
- Multi-select glow effect (iterate `selectedNodes`)
- Selection count badge rendering
- Box-select rectangle rendering during drag

### Phase 2: Batch Actions UI (~2 sessions)

**Info panel transformation (`info-panel.js`):**
- Detect when `selectedNodes.length > 1`
- Render batch action view instead of single-entity view
- List selected entities with type icons
- Show contextual action buttons

**Action handlers:**
```javascript
async function batchAddToQueue(nodeIds) {
    for (const id of nodeIds) {
        await connection.queueAdd(id);
    }
    showToast(`Added ${nodeIds.length} items to queue`);
}

async function batchClose(nodeIds, reason) {
    for (const id of nodeIds) {
        await connection.closeEntity(id, reason);
    }
    showToast(`Closed ${nodeIds.length} items`);
}
```

### Phase 3: Relationship Builder (~2 sessions)

**New component (`link-builder.js`):**
- Modal/panel for creating links between selected entities
- Link type dropdown (depends_on, blocks, related_to, child_of, etc.)
- Direction swap button
- Chain/hub/mesh mode for 3+ entities
- Preview visualization before committing
- Batch link creation via API

**API requirements:**
- `POST /api/link` already exists
- May need batch endpoint: `POST /api/links/batch` for efficiency

### Phase 4: Polish (~1 session)

- Keyboard shortcuts (Ctrl+A, Escape, etc.)
- Selection persistence across view changes
- Undo support for batch operations
- Animation/transitions for selection state changes
- Mobile-friendly selection (long-press?)

## API Additions

### Batch Link Creation
```
POST /api/links/batch
{
  "links": [
    { "from": "bn-a1b2", "to": "bn-c3d4", "type": "depends_on", "reason": "..." },
    { "from": "bn-c3d4", "to": "bn-e5f6", "type": "depends_on", "reason": "..." }
  ]
}
```

### Batch Status Update
```
POST /api/entities/batch/status
{
  "ids": ["bn-a1b2", "bn-c3d4", "bn-e5f6"],
  "status": "closed",
  "reason": "Completed in sprint 12"
}
```

### Batch Queue Operations
```
POST /api/queue/batch/add
{
  "ids": ["bn-a1b2", "bn-c3d4"]
}

POST /api/queue/batch/remove
{
  "ids": ["bn-a1b2", "bn-c3d4"]
}
```

## Technical Considerations

### Performance
- Selection state changes should be O(1) for toggle, O(n) for clear
- Rendering many selected nodes shouldn't impact frame rate
- Consider Set instead of Array for `selectedNodes` if lookups frequent

### Accessibility
- Selection state announced to screen readers
- Keyboard-only selection must be possible
- Focus management for multi-select actions

### Readonly Mode
- Selection should still work (for viewing/comparing)
- Batch action buttons disabled/hidden
- Export still allowed

### Mobile/Touch
- Long-press to toggle selection
- Two-finger tap for range select?
- Consider simplified batch actions for touch

## Testing

### Unit Tests
- Selection state management (toggle, clear, selectAll)
- Modifier key detection
- Box-select geometry calculations

### Integration Tests
- Ctrl+click adds to selection
- Shift+drag creates box select
- Batch queue add/remove works
- Link builder creates correct relationships

### Manual Testing
- Select 10+ entities, verify no performance issues
- Create chain links, verify graph updates correctly
- Test on various screen sizes
- Verify readonly mode restrictions

## Success Criteria

1. Users can select multiple entities with standard Ctrl/Shift+click patterns
2. Box select allows rapid multi-selection
3. Batch queue operations work reliably
4. Relationship builder creates valid links
5. No performance regression with large selections
6. Readonly mode properly restricts actions

## Estimated Effort

**Medium-Large** (6-8 focused sessions)
- Phase 1: 2-3 sessions (selection infrastructure)
- Phase 2: 2 sessions (batch actions UI)
- Phase 3: 2 sessions (relationship builder)
- Phase 4: 1 session (polish)

## Dependencies

- Existing info panel component
- Link creation API (`bn link add`)
- Queue management API

## Future Enhancements

- **Smart selection**: "Select all blocked tasks", "Select dependencies of X"
- **Selection sets**: Save named selections for later use
- **Drag-to-link**: Drag from one selected entity to another to create link
- **Comparison view**: Side-by-side diff of selected entities
- **Bulk edit**: Edit shared fields across selected entities
- **Selection history**: Undo/redo selection changes
- **Visual grouping**: Temporarily cluster selected nodes visually
- **Export templates**: Customizable export formats (Jira, Linear, etc.)
