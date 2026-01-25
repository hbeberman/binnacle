# PRD: Clickable Entity IDs in Markdown

**Related ideas:** `bn-4b5c` (PRDs as document nodes with traceability)

## Overview

Make binnacle entity IDs (`bn-xxxx`, `bnt-xxxx`, `bnq-xxxx`) in rendered markdown content clickable, navigating users to that node's location on the graph view. This creates intuitive cross-referencing between documentation and the visual task graph.

## Problem Statement

Currently, when viewing doc nodes, PRDs, task descriptions, or other markdown content in the GUI:
- Entity IDs appear as plain text with no interactivity
- Users must manually search or browse to find referenced entities
- The connection between documentation and the graph is cognitive, not navigable
- The `linkifyEntityIds()` function exists and works in activity logs, but isn't applied to markdown rendering

## Proposed Solution

Extend markdown rendering to automatically detect and linkify binnacle entity IDs, using the existing `linkifyEntityIds()` infrastructure.

### Scope

**In scope:**
- Doc viewer content (both summary and main content sections)
- Task/bug/idea description fields when rendered as markdown
- Any future markdown rendering contexts

**Out of scope (for now):**
- Hover previews/tooltips showing entity details
- Creating new IDs inline (editor functionality)
- External markdown files outside the GUI

### Behavior

1. **Detection**: Match `bn-xxxx`, `bnt-xxxx`, `bnq-xxxx` patterns (4 hex chars) in rendered content
2. **Rendering**: Display matched IDs as styled clickable elements (pill/badge style)
3. **Click action**: Navigate to that node on the graph view
4. **Modal handling**: If the doc viewer modal is open, close it before navigating
5. **Non-existent IDs**: Style differently (muted/strikethrough) to indicate the reference is stale or invalid

### Visual Design

**Valid entity ID:**
```
┌─────────────┐
│ 📋 bn-a1b2  │  ← Pill with entity-type icon, colored background
└─────────────┘
```

**Invalid/non-existent entity ID:**
```
┌─────────────┐
│ ⚠️ bn-dead  │  ← Muted style, warning indicator
└─────────────┘
```

Entity type icons (reuse existing):
- Tasks: 📋
- Bugs: 🐛  
- Ideas: 💡
- Tests: 🧪
- Queues: 📥
- Docs: 📄
- Unknown: ❓

### CSS Classes

```css
.entity-link {
  display: inline-flex;
  align-items: center;
  gap: 0.25em;
  padding: 0.1em 0.5em;
  border-radius: 4px;
  background: var(--entity-link-bg);
  color: var(--entity-link-color);
  cursor: pointer;
  font-family: monospace;
  font-size: 0.9em;
  text-decoration: none;
  transition: background 0.15s;
}

.entity-link:hover {
  background: var(--entity-link-hover-bg);
}

.entity-link.invalid {
  background: var(--entity-link-invalid-bg);
  color: var(--entity-link-invalid-color);
  cursor: not-allowed;
  text-decoration: line-through;
}
```

## Implementation

### Phase 1: Apply linkifyEntityIds to Markdown

1. Modify `renderMarkdown()` to call `linkifyEntityIds()` as a final post-processing step
2. Ensure HTML escaping doesn't break the linkified spans
3. Protect code blocks from linkification (IDs in code should remain plain text)

```javascript
function renderMarkdown(markdown) {
    // ... existing markdown processing ...
    
    // After all other transformations, linkify entity IDs
    // (but not inside code blocks)
    html = linkifyEntityIdsPreservingCode(html);
    
    return html;
}
```

### Phase 2: Click Handler Enhancement

1. Enhance the existing `.entity-link` click handler to:
   - Close any open modal (doc viewer, info panel, etc.)
   - Switch to graph view
   - Pan to and select the target node
   - Apply temporary highlight animation

```javascript
document.addEventListener('click', (e) => {
    const link = e.target.closest('.entity-link');
    if (!link) return;
    
    const entityId = link.dataset.entityId;
    if (!entityId) return;
    
    // Close doc viewer modal if open
    closeDocViewer();
    
    // Navigate to entity
    navigateToEntity(entityId);
});
```

### Phase 3: Validation and Styling

1. Enhance `linkifyEntityIds()` to check entity existence and apply appropriate styling
2. Add `invalid` class to non-existent entities
3. Add entity-type icon prefix based on ID prefix

```javascript
function linkifyEntityIds(text) {
    const entityPattern = /\b(bn-[a-f0-9]{4}|bnt-[a-f0-9]{4}|bnq-[a-f0-9]{4})\b/gi;
    return text.replace(entityPattern, (match) => {
        const id = match.toLowerCase();
        const exists = entityExists(id);
        const icon = getEntityIcon(id);
        const validClass = exists ? '' : ' invalid';
        return `<span class="entity-link${validClass}" data-entity-id="${id}">${icon} ${match}</span>`;
    });
}

function getEntityIcon(id) {
    if (id.startsWith('bnt-')) return '🧪';
    if (id.startsWith('bnq-')) return '📥';
    // For bn- prefix, need to look up entity type
    const entity = findEntityById(id);
    if (!entity) return '❓';
    switch (entity.type) {
        case 'task': return '📋';
        case 'bug': return '🐛';
        case 'idea': return '💡';
        case 'doc': return '📄';
        default: return '📋';
    }
}
```

## Testing

### Unit Tests
- `linkifyEntityIds()` correctly matches all ID formats
- Code blocks are preserved (IDs inside ``` remain plain)
- Invalid IDs get `invalid` class
- Correct icons assigned per entity type

### Integration Tests
- Click on entity link closes modal and navigates to graph
- Click on invalid entity link shows toast/error (no navigation)
- Multiple IDs in same paragraph all become clickable
- IDs in code blocks remain unlinked

### Manual Testing
- Open a doc with entity references
- Verify IDs are styled as pills
- Click an ID → modal closes, graph view shows, node is highlighted
- Reference a deleted entity → appears with warning styling

## Success Criteria

1. All entity IDs in markdown content are automatically linkified
2. Clicking a valid ID navigates to that node on the graph
3. Invalid references are visually distinct
4. No regression in activity log linkification
5. Code blocks preserve literal ID text

## Estimated Effort

**Small-Medium** (1-2 focused sessions)
- Phase 1: ~30 min (apply existing function to markdown)
- Phase 2: ~30 min (click handler + modal closing)
- Phase 3: ~1 hour (validation, icons, styling)
- Testing: ~30 min

## Dependencies

- None (builds on existing infrastructure)

## Future Enhancements

- Hover tooltip showing entity title/status preview
- Keyboard navigation (Tab through links, Enter to navigate)
- Right-click context menu (open in new tab, copy ID, etc.)
- Bidirectional linking (show "referenced by" in entity info panel)
