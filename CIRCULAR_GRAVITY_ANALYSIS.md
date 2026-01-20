# Circular Gravity Canvas - Implementation Analysis

## Current Implementation Summary

**File:** `src/gui/index.html`

### Architecture Overview

The current graph visualization is a single-page application with:
- Canvas-based rendering (lines 109-115, 435-447)
- Spring physics layout system (lines 335-339, 699-812)
- Interactive node dragging (lines 340-352, 459-613)
- Real-time WebSocket updates (lines 355-380)

### Current Physics System

**Spring Forces** (`applySpringForces`, lines 724-752):
- Edges create spring connections between nodes
- Ideal distance: 150px (line 736)
- Spring strength: 0.05 (line 337)
- Pulls connected nodes toward ideal separation

**Repulsion Forces** (`applyRepulsionForces`, lines 754-783):
- All node pairs repel each other
- Strength: 5000 (line 338)
- Inverse-square law: `force = strength / distance²` (line 767)
- Prevents node overlap

**Position Updates** (`updatePositions`, lines 785-812):
- Velocity damping: 0.9 (line 336)
- Boundary enforcement at canvas edges (lines 805-810)
- Velocity zeroed when hitting boundaries

### Node Dragging System

**Drag State** (lines 340-352):
```javascript
draggedNode: null,        // Currently dragged node
isDragging: false,        // Drag active flag
dragStartX/Y: 0,          // Mouse position at drag start
dragNodeStartX/Y: 0,      // Node position at drag start
dragLastX/Y: 0,           // For momentum calculation
```

**Drag Behavior:**
- Nodes freeze during drag (no physics applied, lines 791-795)
- Position clamped to canvas boundaries (lines 479-483)
- Momentum applied on release (lines 561-566): `velocity = dragDelta * 2.5`
- Tooltip follows dragged node (line 490)

### Mouse Event Handlers

**Current handlers** (lines 459-613):
- `mousemove`: hover detection, drag position updates, tooltip positioning (465-510)
- `mouseleave`: clear drag/hover state (512-525)
- `mousedown`: initiate drag on node click (527-553)
- `mouseup`: end drag, apply momentum (555-584)
- `dblclick`: toggle node selection (586-612)

### Boundary Constraints

**Two enforcement points:**
1. **During drag** (lines 479-483):
   ```javascript
   const margin = node.radius + 10;
   node.x = Math.max(margin, Math.min(canvas.width - margin, node.x));
   node.y = Math.max(margin, Math.min(canvas.height - margin, node.y));
   ```

2. **During physics** (lines 805-810):
   ```javascript
   if (node.x < margin) { node.x = margin; node.vx = 0; }
   if (node.x > canvas.width - margin) { ... }
   // Similar for y-axis
   ```

## Changes Required for Circular Gravity + Infinite Canvas

### 1. Viewport Transformation System (Task bn-c9c7)

**Priority: 1 - Must implement first**

Add to `state.graph`:
```javascript
viewport: {
    panX: 0,           // World space offset
    panY: 0,
    zoom: 1.0,         // Current zoom level
    minZoom: 0.1,      // Zoom limits
    maxZoom: 3.0
}
```

**Coordinate transformations:**
```javascript
function screenToWorld(screenX, screenY) {
    return {
        x: (screenX - canvas.width/2) / zoom - panX,
        y: (screenY - canvas.height/2) / zoom - panY
    };
}

function worldToScreen(worldX, worldY) {
    return {
        x: (worldX + panX) * zoom + canvas.width/2,
        y: (worldY + panY) * zoom + canvas.height/2
    };
}
```

**Apply to rendering:**
- Transform all `ctx.arc()`, `ctx.moveTo()`, `ctx.lineTo()` calls
- Scale node radii by zoom
- Scale font sizes by zoom
- Transform tooltip positioning

### 2. Infinite Canvas Implementation (Task bn-3ec3)

**Priority: 1 - Depends on bn-c9c7**

**Remove boundary constraints:**
- Delete lines 479-483 (drag boundary clamping)
- Delete lines 805-810 (physics boundary clamping)

**Add canvas panning:**
- Detect pan gesture (middle-mouse or space+drag)
- Distinguish from node drag
- Update `viewport.panX/panY` during pan
- Prevent simultaneous node drag and canvas pan

**Culling optimization:**
- Calculate visible world bounds from viewport
- Only render nodes within visible area + margin
- Continue physics for all nodes (or use spatial partitioning)

### 3. Circular Gravity Physics (Tasks bn-5092, bn-b2e7)

**Priority: 1 - Design first (bn-5092), then implement (bn-b2e7)**

**Design parameters (bn-5092):**
- Gravity center point (default: world origin 0,0)
- Center gravity strength (suggested: 0.02 - 0.1)
- Ideal orbit radius (suggested: 200-400 units)
- Node repulsion strength (keep current: 5000, or adjust)
- Edge attraction strength (optional: 0.01 - 0.03)

**Physics changes (bn-b2e7):**

Replace `applySpringForces` (lines 724-752) with `applyCircularGravity`:
```javascript
function applyCircularGravity(nodes, physics) {
    const centerX = 0;  // World space center
    const centerY = 0;

    nodes.forEach(node => {
        if (node === draggedNode) return;

        const dx = centerX - node.x;
        const dy = centerY - node.y;
        const distance = Math.sqrt(dx * dx + dy * dy);

        // Gravity toward center
        const force = physics.gravityStrength;
        node.vx += (dx / distance) * force;
        node.vy += (dy / distance) * force;
    });
}
```

Keep `applyRepulsionForces` (lines 754-783) with possible strength adjustment.

Optionally add weak edge attraction:
```javascript
function applyEdgeAttraction(nodes, edges, physics) {
    edges.forEach(edge => {
        const from = nodes.find(n => n.id === edge.from);
        const to = nodes.find(n => n.id === edge.to);
        if (!from || !to) return;

        const dx = to.x - from.x;
        const dy = to.y - from.y;
        const force = physics.edgeAttractionStrength;  // Small value

        from.vx += dx * force;
        from.vy += dy * force;
        to.vx -= dx * force;
        to.vy -= dy * force;
    });
}
```

### 4. Zoom Controls (Task bn-0a4c)

**Priority: 1 - Depends on bn-c9c7**

**Mouse wheel zoom:**
```javascript
canvas.addEventListener('wheel', (e) => {
    e.preventDefault();

    const rect = canvas.getBoundingClientRect();
    const mouseX = e.clientX - rect.left;
    const mouseY = e.clientY - rect.top;

    // World point under cursor before zoom
    const worldBefore = screenToWorld(mouseX, mouseY);

    // Update zoom
    const zoomDelta = e.deltaY > 0 ? 0.9 : 1.1;
    viewport.zoom = Math.max(viewport.minZoom,
                            Math.min(viewport.maxZoom,
                                    viewport.zoom * zoomDelta));

    // World point under cursor after zoom
    const worldAfter = screenToWorld(mouseX, mouseY);

    // Adjust pan to keep point stationary
    viewport.panX += worldBefore.x - worldAfter.x;
    viewport.panY += worldBefore.y - worldAfter.y;
});
```

**UI zoom buttons:**
- Add zoom in/out buttons to nav or overlay
- Call same zoom logic with fixed center point (canvas center)

### 5. Mouse Handler Updates (Task bn-9646)

**Priority: 2 - Depends on bn-3ec3 and bn-0a4c**

**Transform all mouse coordinates:**
- Update hover detection to use world coordinates
- Update drag positions to use world coordinates
- Update tooltip positioning (keep in screen space)

**Pan gesture detection:**
```javascript
let isPanning = false;
let panStartX, panStartY;

canvas.addEventListener('mousedown', (e) => {
    if (e.button === 1 || (e.button === 0 && e.shiftKey)) {
        // Middle mouse or shift+left = pan
        isPanning = true;
        panStartX = e.clientX;
        panStartY = e.clientY;
        e.preventDefault();
    } else {
        // Normal left click = node drag
        const worldPos = screenToWorld(mouseX, mouseY);
        const node = findNodeAt(worldPos.x, worldPos.y);
        if (node) {
            startNodeDrag(node, worldPos);
        }
    }
});
```

**Update tooltip transform:**
- Tooltip position remains in screen space
- But detection uses world coordinates

### 6. Testing (Task bn-7947)

**Priority: 2 - Depends on all above**

**Test cases:**
- [ ] Various node counts (5, 10, 25, 50+ nodes)
- [ ] Circular cloud formation around center
- [ ] Smooth zoom with mouse wheel
- [ ] Smooth pan with middle-mouse/space+drag
- [ ] Node dragging works correctly in world space
- [ ] Nodes don't escape to infinity
- [ ] No node overlap (repulsion working)
- [ ] Performance with 50+ nodes
- [ ] Tooltip accuracy at various zoom levels
- [ ] Tooltip doesn't go off-screen
- [ ] Node selection persists through zoom/pan
- [ ] Edge rendering follows nodes correctly

## Implementation Order

1. **bn-f0ca** ✓ Analysis complete (this document)
2. **bn-5092** Design circular gravity parameters
3. **bn-c9c7** Implement viewport transformation system
4. **bn-3ec3** Remove boundaries, add pan support
5. **bn-0a4c** Add zoom controls
6. **bn-b2e7** Replace spring physics with circular gravity
7. **bn-9646** Update all mouse handlers
8. **bn-7947** Comprehensive testing

## Key Design Decisions Needed

1. **Gravity center**: Fixed at world origin (0,0) or dynamic?
2. **Gravity strength**: What value creates stable circular cloud?
3. **Edge attraction**: Include or rely only on gravity + repulsion?
4. **Pan gesture**: Middle-mouse, space+drag, or both?
5. **Initial zoom**: Start at 1.0 or auto-fit to show all nodes?
6. **Zoom limits**: 0.1-3.0x reasonable or need wider range?

## Files to Modify

- `src/gui/index.html` - All changes in this single file
  - Lines 329-352: Add viewport state
  - Lines 459-613: Update mouse handlers
  - Lines 689-722: Update animation loop
  - Lines 724-812: Replace physics system
  - New functions: coordinate transforms, circular gravity, zoom handler

## Risks & Considerations

1. **Performance**: Infinite canvas with many nodes needs culling
2. **UX**: Users need visual feedback about pan/zoom state
3. **Stability**: Circular gravity parameters must prevent runaway nodes
4. **Discoverability**: Pan/zoom controls should be obvious
5. **Touch support**: Future consideration for mobile devices
