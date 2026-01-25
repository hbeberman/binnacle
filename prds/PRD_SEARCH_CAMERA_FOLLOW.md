# PRD: Search-Driven Camera Follow

## Summary

Enhance the GUI graph view so that when the search bar has text, the camera automatically zooms to fit all matching nodes. This provides an intuitive "follow the search" experience that overrules the existing follow dropdown setting while search is active.

**Related ideas:** bn-d96c (Enhanced Auto-Follow Mode with Configurable Node Tracking)

---

## Problem

Users can search for nodes in the graph, but the camera doesn't respond to the search results. After typing a query, users must manually pan and zoom to find the matching nodes. This is especially tedious in large graphs where matches may be scattered across the canvas.

The current follow dropdown (`Auto`, `Active Task`, `Queue`, agent IDs) only tracks individual entities, not search results.

---

## Goals

1. When the search bar contains text, the camera smoothly zooms to fit all matching nodes
2. Search-driven camera overrules the follow dropdown setting (but preserves it)
3. When search is cleared, the previous follow mode resumes seamlessly
4. Clear visual feedback when search has no matches
5. Debounced input to avoid camera jitter while typing

## Non-Goals

- Adding a "Search" option to the follow dropdown (search implicitly takes over)
- Modifying the follow dropdown behavior itself
- Persisting search queries across sessions
- Advanced search syntax (regex, filters, etc.)

---

## UX Design

### Search → Camera Behavior

| State | Camera Behavior |
|-------|-----------------|
| Search empty | Normal follow mode (dropdown setting applies) |
| Search has matches | Zoom to fit all matches (smooth animation) |
| Search has no matches | Stay at current position, show "no matches" overlay |
| Search cleared | Resume previous follow mode |

### Animation

- **Easing curve:** Ease-out (fast start, gentle landing) for responsive feel
- **Duration:** Adaptive based on viewport change magnitude
  - Small adjustments: 200ms
  - Medium repositions: 400ms
  - Large jumps: 600ms
- **Debounce delay:** 500ms (balanced - waits for typing to settle)

### No Matches State

When the search query yields zero matches:

1. **Graph overlay:** Subtle futuristic overlay appears
   - Semi-transparent dark background over the entire graph (slight grey-out effect, ~20% opacity)
   - Centered text: "No matches" in a sleek, minimal font style
   - Optional: subtle scan-line or grid pattern animation for futuristic feel
   
2. **Search input:** Border turns orange-red to indicate no results
   - Use `#e05252` or similar warm warning color
   - Returns to normal border on any match or when cleared

3. **Camera:** Stays at current position (no jarring resets)

### Visual Hierarchy

```
┌─────────────────────────────────────────────────────────┐
│ [Search nodes…_______] [Follow: Auto ▼] [⚙️] [Zoom +/-] │
│                                                         │
│                    ┌───────────────┐                    │
│                    │               │                    │
│    (greyed out)    │  No matches   │    (greyed out)   │
│                    │               │                    │
│                    └───────────────┘                    │
│                                                         │
└─────────────────────────────────────────────────────────┘
```

---

## Technical Design

### State Management

Add to `state.graph`:

```javascript
state.graph = {
  // ... existing properties
  
  searchCameraActive: false,       // True when search is driving camera
  searchDebounceTimer: null,       // Timer ID for debouncing
  previousFollowTargetId: null,    // Saved follow setting to restore
  noMatchesOverlayVisible: false,  // Show the "no matches" overlay
};
```

### Core Logic

```javascript
// Pseudocode for search input handler
function onSearchInput(query) {
  clearTimeout(state.graph.searchDebounceTimer);
  
  if (!query.trim()) {
    // Search cleared - restore previous follow mode
    exitSearchCameraMode();
    return;
  }
  
  state.graph.searchDebounceTimer = setTimeout(() => {
    const matches = getMatchingNodes(query);
    
    if (matches.length === 0) {
      showNoMatchesState();
    } else {
      hideNoMatchesState();
      zoomToFitNodes(matches);
    }
  }, 500); // 500ms debounce
}

function enterSearchCameraMode() {
  if (!state.graph.searchCameraActive) {
    state.graph.previousFollowTargetId = state.graph.followTargetId;
    state.graph.searchCameraActive = true;
  }
}

function exitSearchCameraMode() {
  state.graph.searchCameraActive = false;
  state.graph.noMatchesOverlayVisible = false;
  state.graph.followTargetId = state.graph.previousFollowTargetId || 'auto';
  // Resume normal follow behavior
  snapToFollowTarget();
}

function zoomToFitNodes(nodes) {
  enterSearchCameraMode();
  
  // Calculate bounding box of all matching nodes
  const bounds = calculateBoundingBox(nodes);
  
  // Add padding (10% on each side)
  const paddedBounds = expandBounds(bounds, 1.2);
  
  // Animate camera to fit bounds
  animateCameraToFitBounds(paddedBounds, {
    easing: 'ease-out',
    duration: calculateAdaptiveDuration(paddedBounds)
  });
}
```

### Zoom-to-Fit Algorithm

```javascript
function animateCameraToFitBounds(bounds, options) {
  const canvas = document.getElementById('graph-canvas');
  const canvasWidth = canvas.width;
  const canvasHeight = canvas.height;
  
  // Calculate required zoom to fit bounds
  const boundsWidth = bounds.maxX - bounds.minX;
  const boundsHeight = bounds.maxY - bounds.minY;
  
  const zoomX = canvasWidth / boundsWidth;
  const zoomY = canvasHeight / boundsHeight;
  const targetZoom = Math.min(zoomX, zoomY, state.graph.zoom); // Don't zoom in past current level
  
  // Calculate center point
  const targetX = (bounds.minX + bounds.maxX) / 2;
  const targetY = (bounds.minY + bounds.maxY) / 2;
  
  // Animate with ease-out
  animateCamera({
    targetX,
    targetY,
    targetZoom,
    easing: easeOut,
    duration: options.duration
  });
}

function easeOut(t) {
  return 1 - Math.pow(1 - t, 3); // Cubic ease-out
}
```

### CSS Additions

```css
/* No matches overlay */
.no-matches-overlay {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(20, 20, 30, 0.2);
  display: flex;
  align-items: center;
  justify-content: center;
  pointer-events: none;
  opacity: 0;
  transition: opacity 0.3s ease;
  z-index: 10;
}

.no-matches-overlay.visible {
  opacity: 1;
}

.no-matches-overlay .message {
  font-family: 'Inter', -apple-system, sans-serif;
  font-size: 1.2rem;
  font-weight: 300;
  letter-spacing: 0.1em;
  color: rgba(255, 255, 255, 0.6);
  text-transform: uppercase;
  padding: 1rem 2rem;
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 4px;
  background: rgba(0, 0, 0, 0.3);
  backdrop-filter: blur(4px);
}

/* Search input no-match state */
.graph-search.no-matches {
  border-color: #e05252 !important;
  box-shadow: 0 0 0 1px rgba(224, 82, 82, 0.3);
}
```

### HTML Additions

```html
<!-- Inside #graph-canvas-container -->
<div class="no-matches-overlay" id="no-matches-overlay">
  <div class="message">No matches</div>
</div>
```

---

## Integration with Existing Features

### Follow Dropdown Interaction

- The follow dropdown remains visible and functional
- When search is active, the dropdown value is preserved but its effect is suspended
- Visual indicator (optional): Dropdown could show a subtle "paused" state while search is active
- When search is cleared, the dropdown setting immediately takes effect

### Graph Filtering

The existing graph search filter (from PRD_GUI_GRAPH_SEARCH_FILTER.md) hides non-matching nodes. This PRD adds camera movement on top of that:

1. User types in search
2. Non-matching nodes are hidden (existing behavior)
3. Camera zooms to fit visible nodes (new behavior)

### Auto-Follow Pause

If auto-follow is enabled and the user types a search:
- Auto-follow is temporarily suspended
- Search camera takes over
- When search is cleared, auto-follow resumes from its saved state

---

## Implementation Tasks

### Phase 1: Core Infrastructure
- [ ] Add search camera state properties to `state.graph`
- [ ] Implement `calculateBoundingBox()` for node collections
- [ ] Implement `animateCameraToFitBounds()` with ease-out easing
- [ ] Add debounced search input handler

### Phase 2: No Matches UI
- [ ] Create no-matches overlay HTML element
- [ ] Add CSS for overlay (futuristic style, grey-out effect)
- [ ] Add CSS for search input no-match state (orange-red border)
- [ ] Wire up show/hide logic for overlay

### Phase 3: Follow Mode Integration
- [ ] Save previous follow target when search activates
- [ ] Restore follow target when search clears
- [ ] Ensure normal follow mode works correctly after search exit
- [ ] Handle edge case: search cleared while graph data updates

### Phase 4: Polish & Edge Cases
- [ ] Test with single match (should center, not over-zoom)
- [ ] Test with matches at graph extremes (large bounding box)
- [ ] Test rapid typing (debounce should prevent jitter)
- [ ] Test clearing search via Escape key
- [ ] Verify performance with 500+ nodes

---

## Acceptance Criteria

- [ ] Typing in search bar triggers camera zoom-to-fit after 500ms debounce
- [ ] Camera smoothly animates with ease-out curve
- [ ] All matching nodes are visible after animation completes
- [ ] Zero matches shows overlay and turns search border orange-red
- [ ] Clearing search restores previous follow mode
- [ ] No camera movement during active typing (debounce works)
- [ ] Follow dropdown setting is preserved through search/clear cycle
- [ ] Works correctly with all existing follow modes (Auto, Active Task, Queue, agents)

---

## Test Plan

### Unit Tests (if applicable)
- `calculateBoundingBox()` returns correct bounds for various node positions
- `easeOut()` interpolation function produces correct values
- Debounce timer correctly delays execution

### Manual Testing
1. Open GUI, type partial task ID → camera should zoom to matching tasks
2. Type query with no matches → overlay appears, border turns orange-red
3. Clear search → camera returns to follow target
4. Set follow to "Active Task", search, clear → follow should track active task again
5. Type rapidly → camera should only move once typing stops

---

## Open Questions

1. Should there be a minimum zoom level when fitting multiple nodes? (Prevent zooming out too far on scattered matches)
2. Should the padding around fitted nodes be configurable?
3. Should Escape key also trigger search clear (in addition to backspacing)?

---

## Future Enhancements

- Keyboard shortcut to focus search bar (e.g., `/` or `Ctrl+F`)
- Search history dropdown for recent queries
- Highlight matching text within node labels
- "Zoom to selection" button that fits selected nodes (not just search matches)
