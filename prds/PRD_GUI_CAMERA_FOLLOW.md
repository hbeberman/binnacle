# PRD: GUI Camera Follow & Stats Overlay

## Overview

Enhance the binnacle web GUI graph view with:
1. **Auto-follow camera** - Smoothly tracks the currently active task as work progresses
2. **Stats overlay** - Real-time counts of bugs, tasks (blocked/ready), and milestones
3. **Toast notification system** - Reusable alerts for completion events and status updates

## Motivation

When agents are actively working through tasks, users want to visually track progress without manually panning the canvas. The auto-follow feature provides a "watch mode" experience. The stats overlay gives at-a-glance project health metrics, and clickable filters help focus attention on specific item types.

---

## Feature 1: Auto-Follow Camera

### Description

A toggle switch in the graph view top bar that, when enabled, automatically pans the camera to center on any task that transitions to `in_progress` status.

### UI Placement

- **Location**: Near the zoom controls (right side of top bar, inside the `#graph-view` container)
- **Control type**: Circular toggle switch (green when ON, grey when OFF)
- **Label**: "Follow active task"
- **Default state**: OFF when opening the graph view

### Behavior

#### Triggering

- Camera pans when any task's status changes to `in_progress` (detected via WebSocket)
- The task must exist in the current graph data

#### Animation

- **Easing curve**: Ease-in-out-cubic for smooth, modern feel (slow start → fast middle → slow end)
- **Duration**: Adaptive based on distance
  - Short distances (<300px): 300ms
  - Medium distances (300-800px): 500ms  
  - Long distances (>800px): 800ms
- **Zoom**: Preserves current zoom level (does not auto-zoom)

#### Interaction Constraints

While auto-follow is enabled:
- **Disabled**: Canvas panning (click-drag on empty space, middle-mouse drag, shift+click drag)
- **Enabled**: Node hovering, node clicking/selection, zoom controls, info panel interaction

#### Completion State

When the last ready task is claimed (no more tasks with status `pending` or `ready`):
1. Display toast notification: "🎉 All tasks completed!"
2. Zoom out by 10% from current level
3. Auto-follow remains enabled but idle

### Technical Notes

- Store follow state in `state.graph.autoFollow` (boolean)
- Track animation with `state.graph.followAnimation` (object with target, progress, startTime)
- Use `requestAnimationFrame` for smooth interpolation
- Detect `in_progress` transitions in WebSocket message handler

---

## Feature 2: Stats Overlay

### Description

A real-time statistics display showing counts of open bugs, tasks (blocked/ready), and milestones.

### UI Placement

- **Location**: To the right of the edge types dropdown filter, in the graph view top bar
- **Style**: Inline with existing controls, matching the dark theme

### Display Format

```
🐛 5 bugs | 📋 12 tasks (❌ 3 blocked, ✅ 9 ready) | 🏁 2 milestones
```

- `🐛` for bugs (open bugs only, from `bn bug list`)
- `📋` for total tasks
- `❌` for blocked count (red X emoji)
- `✅` for ready count (green checkmark emoji)
- `🏁` for milestones (open milestones)

### Interactive Behavior

#### Clickable Filters

Each stat segment is clickable:
- Click "bugs" → Highlights all bug nodes, dims others
- Click "blocked" → Highlights blocked tasks, dims others
- Click "ready" → Highlights ready tasks, dims others
- Click "milestones" → Highlights milestone nodes, dims others

#### Highlight Effect

- **Highlighted nodes**: Full opacity, normal rendering
- **Dimmed nodes**: 30% opacity, muted colors
- **Edges**: Dim edges not connected to highlighted nodes

#### Clearing Filter

- Click the same stat again to clear
- Click a different stat to switch filters
- No "clear all" button needed

### Real-Time Updates

- Stats update immediately when WebSocket pushes data changes
- No polling - purely reactive to WebSocket messages
- Recalculate counts after each `tasks`, `bugs`, or `milestones` data update

### Technical Notes

- Add `state.graph.highlightFilter` (null | 'bugs' | 'blocked' | 'ready' | 'milestones')
- Modify `drawNode()` to check filter state and apply opacity
- Count bugs from separate bugs data (not tasks with bug tag)
- Use existing WebSocket data flow

---

## Feature 3: Toast Notification System

### Description

A reusable toast notification system for displaying transient alerts.

### UI Design

- **Position**: Top-center of the graph view, below the header
- **Style**: Dark background with border, rounded corners, shadow
- **Animation**: Fade in from top (slide down), fade out upward
- **Duration**: 5 seconds default, auto-dismiss
- **Dismissible**: X button in top-right corner of toast

### Toast Types

| Type | Icon | Background | Use Case |
|------|------|------------|----------|
| success | ✅ | Green tint | Task completion, all tasks done |
| info | ℹ️ | Blue tint | WebSocket reconnected |
| warning | ⚠️ | Yellow tint | Connection issues |
| error | ❌ | Red tint | Failures |

### API

```javascript
showToast({
  message: "🎉 All tasks completed!",
  type: "success",  // success | info | warning | error
  duration: 5000,   // ms, 0 = no auto-dismiss
  dismissible: true
});
```

### Initial Use Cases

1. **All tasks completed** (auto-follow feature): "🎉 All tasks completed!"
2. **WebSocket reconnected**: "ℹ️ Connection restored"
3. **WebSocket disconnected**: "⚠️ Connection lost, reconnecting..."

### Technical Notes

- Create `toastContainer` div for stacking multiple toasts
- Support multiple simultaneous toasts (stack vertically)
- Auto-dismiss uses `setTimeout`, cancelled on manual dismiss

---

## Implementation Tasks

### Phase 1: Toast System (Foundation)
- [ ] Create toast container HTML/CSS
- [ ] Implement `showToast()` function
- [ ] Add toast animations (enter/exit)
- [ ] Wire up WebSocket connection status toasts

### Phase 2: Stats Overlay
- [ ] Add stats container HTML/CSS to graph view
- [ ] Calculate and display initial counts
- [ ] Wire up real-time updates from WebSocket
- [ ] Implement clickable filter highlighting
- [ ] Add dimming effect to non-highlighted nodes

### Phase 3: Auto-Follow Camera
- [ ] Add toggle switch HTML/CSS near zoom controls
- [ ] Implement follow state management
- [ ] Add ease-in-out-cubic interpolation function
- [ ] Implement adaptive duration calculation
- [ ] Detect `in_progress` transitions in WebSocket handler
- [ ] Disable panning while following (preserve other interactions)
- [ ] Add completion detection and toast
- [ ] Implement 10% zoom-out on completion

### Phase 4: Testing & Polish
- [ ] Test with rapid task transitions
- [ ] Verify zoom level preservation
- [ ] Test filter highlight combinations
- [ ] Verify toast stacking behavior
- [ ] Performance testing with large graphs

---

## Open Questions

1. Should auto-follow also work for bugs transitioning to `in_progress`?
2. Should there be keyboard shortcuts for toggling auto-follow (e.g., `F` key)?
3. Should the stats show percentages as well (e.g., "3/12 blocked")?

---

## Success Criteria

- [ ] Toggle switch enables/disables auto-follow without page refresh
- [ ] Camera smoothly pans to newly active tasks with correct easing
- [ ] Zoom level is preserved during auto-follow pans
- [ ] Stats update in real-time as data changes
- [ ] Clicking stats highlights correct nodes and dims others
- [ ] Toast notifications appear, auto-dismiss, and can be manually closed
- [ ] Performance remains smooth with 200+ nodes
