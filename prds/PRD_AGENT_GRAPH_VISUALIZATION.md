# PRD: Agent Graph Visualization

**Related Ideas:** bn-1f83 (carrier orchestrator), bn-b5c5 (monitor nodes), bn-19aa (arbiter agent), bn-d96c (auto-follow config), bn-7968 (replay mode)  
**Status:** Draft  
**Priority:** P3 (Low)  
**Tags:** gui, agents, visualization, ux

## Problem Statement

Currently, binnacle's GUI only displays **worker agents** on the graph. Other agent types (planner, PRD, buddy/reviewer) are invisible to operators, even though they contribute work to the task graph. This creates blind spots:

1. **No visibility into planning sessions** - When a planner agent researches and outlines a feature, operators can't see this happening
2. **No PRD authorship tracking** - PRD agents create documents but aren't visible on the graph doing it
3. **Incomplete workflow picture** - The graph shows tasks being done but not the full lifecycle of how work gets planned → specified → executed
4. **Can't focus on a specific agent's work** - No way to filter the view to see just what one agent touched

The GUI should show all agent types as first-class graph nodes, with visual affordances that help operators understand multi-agent workflows.

## Solution Overview

Extend the binnacle GUI to:

1. **Show all agent types as graph nodes** (not just workers)
2. **Add agent type badges** for visual differentiation
3. **Enable filtering by agent type** (planner, prd, task, general, buddy)
4. **Implement agent focus mode** - center camera on an agent and show only nodes it touched
5. **Smooth camera transitions** when following agents or switching focus
6. **Fade-out animation** when agents terminate (6-second fade after `bn goodbye`)

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Agent visual style | Unified appearance with type badge | Keeps graph visually consistent; badge provides differentiation |
| Agent types to show | All (`planner`, `prd`, `task`, `general`, `buddy`) | Full visibility into multi-agent workflows |
| Focus mode behavior | Camera centers on agent, dims unrelated nodes | Clear focus without losing context |
| Fade duration | 6 seconds after `bn goodbye` | Long enough to notice, short enough not to clutter |
| Session persistence | Agents stored in graph data | Enables querying past activity, history review |
| Camera transitions | Smooth pan (ease-in-out) | Visually pleasing, easy to follow |

## Detailed Design

### 1. Agent Type Registration

Extend `bn orient` to accept an `--type` flag that records the agent's role:

```bash
bn orient --type planner    # Planning/research agent
bn orient --type prd        # PRD authoring agent  
bn orient --type task       # Implementation agent (default, current "worker")
bn orient --type general    # General-purpose agent
bn orient --type buddy      # Review/assistance agent
```

**Data model change** (`agents.jsonl`):

```json
{
  "id": "bn-e87e",
  "type": "agent",
  "pid": 12345,
  "parent_pid": 12340,
  "name": "claude-planner-1",
  "agent_type": "planner",
  "started_at": "2026-01-25T03:00:00Z",
  "ended_at": null,
  "last_activity_at": "2026-01-25T03:15:00Z",
  "tasks": [],
  "docs": ["bn-4f2a"],
  "command_count": 42
}
```

New fields:
- `agent_type` - One of: `planner`, `prd`, `task`, `general`, `buddy`
- `ended_at` - Timestamp when `bn goodbye` was called (null if active)
- `docs` - Document nodes created/edited by this agent

### 2. GUI: All Agent Types on Graph

**Current behavior:** Only agents with `agent_type === 'worker'` are rendered.

**New behavior:** All agents are rendered, with visual differentiation.

#### Agent Node Appearance

All agents share a unified base style (same shape/size as current worker nodes), but with:

1. **Type badge** - Small icon/label in corner indicating agent type:
   - 📋 Planner
   - 📄 PRD
   - ⚙️ Task (worker)
   - 🔧 General
   - 👀 Buddy

2. **Status indicator** - Ring color shows activity state:
   - 🟢 Green ring - Active (recent activity within 5 min)
   - 🟡 Yellow ring - Idle (no activity >5 min)
   - ⚪ Fading out - Terminated, in 6-second fade

3. **Edges from agents:**
   - `working_on` → Task nodes (existing)
   - `created` → Doc/PRD nodes authored by agent
   - `reviewed` → Nodes the agent reviewed (buddy agents)

### 3. Agent Type Filtering

Add agent type toggles to the existing filter panel (alongside node type filters).

```
┌─ Agent Types ─────────────────┐
│ ☑ 📋 Planner                  │
│ ☑ 📄 PRD                      │
│ ☑ ⚙️ Task                     │
│ ☑ 🔧 General                  │
│ ☑ 👀 Buddy                    │
└───────────────────────────────┘
```

**Filter behavior:**
- Unchecking a type hides those agent nodes and their edges
- Filter state persists in localStorage
- Independent of existing node type filters (tasks, bugs, docs, etc.)

### 4. Agent Focus Mode

New UI control: Click an agent node (or select from dropdown) to enter **focus mode**.

**Focus mode behavior:**
1. Camera smoothly pans to center on the selected agent
2. Nodes the agent hasn't touched are **dimmed** (50% opacity)
3. "Touched" means: created, claimed, updated, or linked to
4. Agent's related nodes remain fully visible
5. Camera follows the agent if it moves (claims new tasks, etc.)
6. Exit focus mode via "X" button or clicking empty space

**Visual treatment in focus mode:**
```
┌─────────────────────────────────────────┐
│  Focused on: 📋 claude-planner-1   [X]  │
│  Touched: 3 tasks, 1 doc                │
└─────────────────────────────────────────┘
```

**Touched node detection:**
- Tasks where agent is/was assignee
- Documents created by agent
- Nodes where agent is in the `created_by` or `updated_by` audit trail
- Nodes linked via edges where agent was source of the link action

### 5. Camera Transitions

Smooth camera panning using ease-in-out interpolation:

```javascript
// Camera transition parameters
const CAMERA_TRANSITION_DURATION = 500; // ms
const CAMERA_EASING = 'ease-in-out';

function panToNode(nodeId) {
  const node = findNode(nodeId);
  const targetX = node.x;
  const targetY = node.y;
  
  animateCamera({
    from: { x: camera.x, y: camera.y },
    to: { x: targetX, y: targetY },
    duration: CAMERA_TRANSITION_DURATION,
    easing: CAMERA_EASING
  });
}
```

Transitions occur when:
- Entering focus mode on an agent
- Agent claims a new task (camera follows)
- User clicks "Go to node" in sidebar
- Switching between followed agents

### 6. Agent Termination Fade-Out

When `bn goodbye` is called:

1. Server broadcasts WebSocket event: `agent_terminated`
2. GUI receives event with `agent_id` and `ended_at` timestamp
3. Agent node begins 6-second fade-out animation:
   - Opacity: 1.0 → 0.0 over 6 seconds
   - Node shrinks slightly (scale 1.0 → 0.8)
   - Edges fade with the node
4. After fade completes, node is removed from render (but persists in storage)

**CSS animation:**
```css
@keyframes agent-fadeout {
  0% { 
    opacity: 1; 
    transform: scale(1); 
  }
  100% { 
    opacity: 0; 
    transform: scale(0.8); 
  }
}

.agent-node.terminating {
  animation: agent-fadeout 6s ease-out forwards;
}
```

### 7. Agent Sidebar Updates

Extend the existing Agents sidebar section to show all agent types:

```
┌─ Agents (4) ──────────────────┐
│ 📋 claude-planner-1  🟢       │
│    Tasks: —  Docs: bn-4f2a    │
│ 📄 claude-prd-2      🟢       │
│    Tasks: —  Docs: bn-7c91    │
│ ⚙️ claude-task-3     🟢       │
│    Tasks: bn-a1b2             │
│ 👀 claude-buddy-4    🟡 idle  │
│    Reviewed: bn-a1b2          │
└───────────────────────────────┘
```

Clicking an agent in the sidebar enters focus mode for that agent.

## Implementation Plan

### Phase 1: Agent Type Infrastructure
- [ ] Add `--type` flag to `bn orient` command
- [ ] Update `Agent` model with `agent_type` field
- [ ] Update `agents.jsonl` schema
- [ ] Add `ended_at` timestamp field
- [ ] Track doc creation/editing per agent
- [ ] Update MCP tools to include agent type

### Phase 2: GUI - Show All Agent Types
- [ ] Remove `agent_type === 'worker'` filter from graph rendering
- [ ] Add type badge rendering to agent nodes
- [ ] Update agent node styling (status ring colors)
- [ ] Add new edge types for agent→doc relationships
- [ ] Update sidebar to show all agent types with their work

### Phase 3: Agent Type Filtering
- [ ] Add agent type filter panel to GUI
- [ ] Implement filter toggle logic
- [ ] Persist filter state in localStorage
- [ ] Update `nodeTypeFilters` to include agent subtypes

### Phase 4: Agent Focus Mode
- [ ] Add focus mode state to GUI
- [ ] Implement node dimming for non-touched nodes
- [ ] Add focus mode UI banner
- [ ] Track "touched" nodes per agent (from audit log)
- [ ] Implement click-to-focus on agent nodes
- [ ] Add exit focus mode controls

### Phase 5: Camera Transitions
- [ ] Implement smooth camera pan animation
- [ ] Add easing function for transitions
- [ ] Integrate with focus mode entry
- [ ] Integrate with agent follow mode
- [ ] Add transition when agent claims new task

### Phase 6: Agent Fade-Out Animation
- [ ] Add `agent_terminated` WebSocket event
- [ ] Implement 6-second fade-out animation
- [ ] Handle edge fading with node
- [ ] Clean up node from render after fade
- [ ] Handle case where terminated agent is in focus mode

## Testing Strategy

### Unit Tests
- Agent model serialization with new fields
- `--type` flag parsing and validation
- "Touched" node calculation logic
- Camera animation interpolation math

### Integration Tests
- `bn orient --type planner` creates correct agent type
- All agent types appear in `/api/graph` response
- Agent type filtering works via API
- `bn goodbye` triggers termination event
- Fade-out timing is correct

### Manual/Visual Tests
- All agent types render on graph with badges
- Focus mode dims correct nodes
- Camera transitions are smooth (no jank)
- Fade-out animation looks good
- Filter toggles work correctly
- Sidebar click enters focus mode

## Future Considerations (Out of Scope)

These are explicitly deferred for potential future work:

- **Replay mode** (filed as bn-7968) - Watch an agent's session unfold over time
- **Agent-to-agent edges** - Show when one agent spawns another
- **Live activity streaming** - Show commands as agent executes them (monitor nodes, bn-b5c5)
- **Agent collaboration view** - Multi-agent coordination visualization
- **Agent role colors** - Different base colors per agent type (vs. unified with badge)

## Open Questions

1. **Badge vs. color** - Should agent types use badge icons, subtle color tints, or both?
2. **Focus mode scope** - Should focus mode show 1-hop neighbors of touched nodes, or strictly only touched nodes?
3. **Historical agents** - Should terminated agents be queryable/viewable from some archive view?

---

## Appendix: Related Ideas Summary

| ID | Title | Relevance |
|----|-------|-----------|
| bn-1f83 | Carrier orchestrator node | Future: agents that spawn sub-agents |
| bn-b5c5 | Monitor nodes | Future: live activity display hanging off agents |
| bn-19aa | Arbiter agent role | Governance layer, privileged agent type |
| bn-d96c | Auto-follow config | Camera following behavior, integrates with focus mode |
| bn-7968 | Replay mode | Deferred: watch session unfold over time |
| bn-4b5c | PRDs as doc nodes | Complements: PRD agents create doc nodes |
