# PRD: Enhanced Auto-Follow Mode with Configurable Node Tracking

## Overview

Enhance the existing Auto-follow camera mode with a configuration panel that allows users to customize which node types trigger automatic camera focus, and how long the camera dwells on new nodes before returning to agent tracking.

**Related idea:** bn-d96c

## Motivation

The current Auto mode follows the most recently active agent, which works well for single-agent workflows. However, users want visibility into new activity across the entire graph—new tasks appearing, bugs being filed, ideas being created. Rather than adding multiple Auto modes (Auto-Agent, Auto-Any, etc.), we consolidate into a single configurable Auto mode that lets users tune the behavior to their preferences.

---

## Current State

The existing follow mode implementation includes:
- **Follow selector dropdown** with options: Auto, Active Task, Queue, and individual agents
- **Auto mode** follows the agent with the most recent `last_activity_at`
- **10-second delay** before switching between agents (prevents rapid thrashing)
- **Toast notifications** already exist for new elements appearing

---

## Design

### Single Configurable Auto Mode

Keep the existing "Auto" option in the dropdown but add a **configuration gear icon (⚙️)** next to the selector that opens a settings panel.

### Configuration Panel

A popover/modal panel with toggles for each node type and a duration setting:

```
┌─────────────────────────────────────┐
│ Auto-Follow Settings            [×] │
├─────────────────────────────────────┤
│ Focus on new nodes:                 │
│                                     │
│ [✓] Agents                          │
│ [✓] Tasks                           │
│ [✓] Bugs                            │
│ [ ] Ideas                           │
│ [ ] Documents                       │
│ [ ] Queue                           │
│                                     │
│ Focus duration: [10] seconds        │
│                                     │
│ Resume agent follow after: [10] sec │
└─────────────────────────────────────┘
```

### Node Types

All node types that can appear in the graph should have a toggle:

| Node Type | Default | Description |
|-----------|---------|-------------|
| `agent`   | ON      | New agents connecting |
| `task`    | ON      | New tasks created |
| `bug`     | ON      | New bugs filed |
| `idea`    | OFF     | New ideas created |
| `doc`     | OFF     | New documents attached |
| `queue`   | OFF     | Queue node (typically one per repo) |

**Rationale for defaults:** Agents, tasks, and bugs are high-signal activity that users typically want to see. Ideas and docs are lower-priority and might create noise.

### Multi-Node Appearance Behavior

When multiple new nodes appear simultaneously (e.g., batch import, rapid activity):

1. **Quick zoom** to the first detected new node
2. **Pan through** each subsequent node in sequence (short dwell per node, ~2 seconds)
3. **End on the most recent** node (by `created_at` timestamp)
4. **Resume agent follow** after the configured cooldown (default 10 seconds)

### Timing Parameters

| Parameter | Default | Range | Description |
|-----------|---------|-------|-------------|
| Focus duration | 10s | 3-30s | How long to focus on a single new node |
| Pan-through dwell | 2s | 1-5s | Per-node time when multiple nodes appear |
| Agent resume cooldown | 10s | 5-60s | Delay before returning to agent follow |

Only "Focus duration" is exposed in the UI initially. Pan-through dwell and resume cooldown use sensible defaults.

---

## UI/UX Details

### Gear Icon Placement

```
┌──────────────────────────────────────────────────────────┐
│ ... [Edge Types ▼]  Follow: [Auto ▼] [⚙️] [🔘 ON]  ...   │
└──────────────────────────────────────────────────────────┘
```

- Gear icon appears **between** the selector dropdown and the on/off toggle
- Click opens the configuration panel as a popover (not modal)
- Panel closes when clicking outside or pressing Escape

### Configuration Panel Styling

- Dark theme matching existing UI
- Compact layout (doesn't need to be large)
- Toggles use the same switch style as the auto-follow on/off toggle
- Duration input is a number field with increment/decrement buttons

### State Persistence

Configuration is stored in `localStorage` and persists across browser sessions:

```javascript
localStorage.setItem('binnacle-auto-follow-config', JSON.stringify({
  followAgents: true,
  followTasks: true,
  followBugs: true,
  followIdeas: false,
  followDocs: false,
  followQueue: false,
  focusDurationSec: 10
}));
```

---

## Technical Implementation

### State Management

Add to `state.graph`:

```javascript
state.graph.autoFollowConfig = {
  followAgents: true,
  followTasks: true,
  followBugs: true,
  followIdeas: false,
  followDocs: false,
  followQueue: false,
  focusDurationSec: 10
};
state.graph.newNodeQueue = [];  // Queue of new nodes to pan through
state.graph.panThroughState = null;  // { currentIndex, startTime }
```

### New Node Detection

Modify the WebSocket message handler to detect new nodes:

```javascript
function handleDataUpdate(newTasks, prevTasks) {
  const prevIds = new Set(prevTasks.map(t => t.id));
  const newNodes = newTasks.filter(t => !prevIds.has(t.id));
  
  // Filter by enabled node types
  const config = state.graph.autoFollowConfig;
  const trackableNodes = newNodes.filter(node => {
    switch (node.type) {
      case 'agent': return config.followAgents;
      case 'task': return config.followTasks;
      case 'bug': return config.followBugs;
      case 'idea': return config.followIdeas;
      case 'doc': return config.followDocs;
      case 'queue': return config.followQueue;
      default: return false;
    }
  });
  
  if (trackableNodes.length > 0 && state.graph.autoFollow && state.graph.followTargetId === 'auto') {
    queueNodesForPanThrough(trackableNodes);
  }
}
```

### Pan-Through Animation

```javascript
function queueNodesForPanThrough(nodes) {
  // Sort by created_at so we end on the most recent
  const sorted = [...nodes].sort((a, b) => 
    new Date(a.created_at) - new Date(b.created_at)
  );
  
  state.graph.newNodeQueue = sorted;
  state.graph.panThroughState = { currentIndex: 0, startTime: Date.now() };
  
  // Start panning to first node
  panToNode(sorted[0].id, false);
}

function updatePanThrough() {
  if (!state.graph.panThroughState) return;
  
  const { currentIndex, startTime } = state.graph.panThroughState;
  const queue = state.graph.newNodeQueue;
  const elapsed = Date.now() - startTime;
  const PAN_THROUGH_DWELL_MS = 2000;
  
  if (elapsed >= PAN_THROUGH_DWELL_MS) {
    const nextIndex = currentIndex + 1;
    if (nextIndex < queue.length) {
      // Move to next node
      state.graph.panThroughState = { currentIndex: nextIndex, startTime: Date.now() };
      panToNode(queue[nextIndex].id, false);
    } else {
      // Done with pan-through, focus on most recent (last in sorted list)
      state.graph.panThroughState = null;
      state.graph.newNodeQueue = [];
      state.graph.focusEndTime = Date.now() + (state.graph.autoFollowConfig.focusDurationSec * 1000);
      state.graph.followingNodeId = queue[queue.length - 1].id;
    }
  }
}
```

### Resume Agent Follow

After focus duration expires:

```javascript
function checkFocusExpiry() {
  if (state.graph.focusEndTime && Date.now() >= state.graph.focusEndTime) {
    state.graph.focusEndTime = null;
    // Resume following most recent agent
    const mostRecentAgent = findMostRecentlyActiveAgent();
    if (mostRecentAgent) {
      state.graph.followingNodeId = mostRecentAgent.id;
      panToNode(mostRecentAgent.id, false);
    }
  }
}
```

---

## Implementation Tasks

### Phase 1: Configuration Panel UI
- [ ] Add gear icon button next to follow selector
- [ ] Create popover panel component with toggle switches
- [ ] Add duration input field
- [ ] Implement localStorage persistence
- [ ] Load saved config on page load

### Phase 2: New Node Detection
- [ ] Track previous task list for diff comparison
- [ ] Filter new nodes by type based on config toggles
- [ ] Skip detection when auto-follow is disabled or target is not 'auto'

### Phase 3: Pan-Through Animation
- [ ] Implement node queue for multi-node arrivals
- [ ] Add pan-through state machine (index, timing)
- [ ] Integrate with existing `panToNode()` function
- [ ] Handle edge cases (nodes removed mid-pan-through)

### Phase 4: Focus Duration & Resume
- [ ] Track focus end time when pan-through completes
- [ ] Resume agent follow when focus expires
- [ ] Cancel focus early if user manually interacts

### Phase 5: Testing & Polish
- [ ] Test with rapid node creation (10+ nodes in 1 second)
- [ ] Test config persistence across browser refresh
- [ ] Test interaction between pan-through and manual pan/zoom
- [ ] Performance test with large graphs (200+ nodes)

---

## Success Criteria

- [ ] Gear icon opens/closes configuration panel smoothly
- [ ] Toggle changes take effect immediately (no page refresh)
- [ ] New nodes matching enabled types trigger camera focus
- [ ] Multiple simultaneous nodes pan through in sequence
- [ ] Camera returns to agent follow after configured duration
- [ ] Configuration persists in localStorage
- [ ] Manual pan/zoom interrupts and cancels pan-through
- [ ] Performance remains smooth with frequent node additions

---

## Future Considerations

- **Sound notifications** for new nodes (optional, off by default)
- **Node type priority** (e.g., bugs always interrupt, ideas queue)
- **Follow rules based on node attributes** (e.g., only P0/P1 bugs)
- **Sync config across devices** via binnacle storage
