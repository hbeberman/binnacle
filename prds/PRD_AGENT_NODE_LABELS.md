# PRD: Agent Node Labels

## Overview

Add large white text labels above agent nodes in the graph view, displaying a simplified name for quick identification. Agent nodes currently show only the person silhouette with no text label.

**Related ideas:** None directly related.

## Problem Statement

Agent nodes are visually distinct (person silhouette) but lack identifying text. When multiple agents are active or when viewing the graph, users must hover over agent nodes to see their ID in the tooltip. This makes it hard to quickly identify which agent is which.

## Scope

### In Scope
- Large white text label above agent nodes
- Display custom agent name if set, fallback to agent ID suffix (e.g., "d6ea")
- Bold font, ~16-18px (larger than task labels)
- Consistent positioning above the node

### Out of Scope
- Setting custom agent names (use existing mechanism or separate feature)
- Label editing in the GUI
- Different label styles per agent status

## User Experience

### Display Format
```
    "planner"          (if custom name set)
       👤
       
    "d6ea"             (fallback to ID suffix)
       👤
```

### Visual Specs
- **Font:** Bold, 16-18px
- **Color:** White (#ffffff) with subtle text shadow for readability
- **Position:** Centered above the node, ~10-15px gap from node top
- **Always visible:** No hover required (unlike current tooltip-only approach)

## Technical Design

### Implementation

In `drawNode()` function, after drawing the agent person shape, add text rendering:

```javascript
// After drawing agent node shape
if (node.type === 'agent') {
    const label = node.name || node.id.split('-').pop(); // "d6ea" from "bna-d6ea"
    
    ctx.save();
    ctx.font = 'bold 17px Inter, system-ui, sans-serif';
    ctx.fillStyle = '#ffffff';
    ctx.textAlign = 'center';
    ctx.textBaseline = 'bottom';
    
    // Text shadow for readability
    ctx.shadowColor = 'rgba(0, 0, 0, 0.5)';
    ctx.shadowBlur = 3;
    ctx.shadowOffsetX = 1;
    ctx.shadowOffsetY = 1;
    
    ctx.fillText(label, node.x, node.y - nodeRadius - 8);
    ctx.restore();
}
```

### Data Model

The agent node should support an optional `name` field:
```json
{
  "id": "bna-d6ea",
  "type": "agent",
  "name": "planner",  // optional custom name
  "status": "active",
  ...
}
```

If `name` is not set or empty, display the ID suffix (last 4 chars after "bna-").

## Tasks

1. **Add label rendering to agent nodes** - Draw text above agent silhouette
2. **Extract simplified name** - Custom name or ID suffix fallback
3. **Style the label** - Bold, white, 16-18px with shadow

## Testing

### Manual Testing
- [ ] Agent with custom name shows name above node
- [ ] Agent without name shows ID suffix (e.g., "d6ea")
- [ ] Label is readable over dark and light backgrounds
- [ ] Label doesn't overlap with other elements
- [ ] Multiple agents show distinct labels

## Success Criteria

1. Agent nodes have visible text labels without hovering
2. Labels are large enough to read at normal zoom levels
3. Custom names display when set, ID suffix otherwise
