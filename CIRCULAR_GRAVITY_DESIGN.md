# Circular Gravity Layout System - Design Specification

## Overview

Replace the current spring-based force-directed layout with a circular gravity system where nodes orbit around a center point, creating an organic circular cloud formation.

## Design Goals

1. **Circular Cloud Formation**: Nodes naturally settle into a circular/spherical cloud around a center point
2. **Stable Equilibrium**: System reaches stable state without runaway nodes
3. **Organic Movement**: Natural, flowing motion as nodes settle
4. **Scale Independence**: Works with varying node counts (5-100+ nodes)
5. **Visual Clarity**: Graph structure remains readable in circular layout

## Physics Model

### Current System (Spring-Based)

- Spring forces pull connected nodes toward ideal distance (150px)
- Repulsion pushes all node pairs apart
- Creates tree-like or clustered layouts
- Boundaries prevent dispersion

### New System (Circular Gravity)

- **Central gravity** pulls all nodes toward center point
- **Node repulsion** prevents overlap
- **Optional edge attraction** maintains graph structure
- **No boundaries** - infinite canvas
- Nodes settle into circular orbits at equilibrium radius

## Physics Parameters

### Core Forces

#### 1. Central Gravity
```javascript
gravityStrength: 0.08
gravityCenter: { x: 0, y: 0 }  // World space origin
```

**Rationale:**
- Constant force toward center (not distance-dependent)
- Strength 0.08 provides stable equilibrium at ~250-350 unit radius
- Balances with repulsion to prevent collapse or dispersion
- Lower values (0.04-0.06): larger, looser cloud
- Higher values (0.10-0.15): tighter, more compact cloud

**Force calculation:**
```javascript
const dx = gravityCenter.x - node.x;
const dy = gravityCenter.y - node.y;
const distance = Math.sqrt(dx * dx + dy * dy);

if (distance > 0) {
    const force = gravityStrength;
    node.vx += (dx / distance) * force;
    node.vy += (dy / distance) * force;
}
```

#### 2. Node Repulsion
```javascript
repulsionStrength: 5000  // Keep current value
```

**Rationale:**
- Same as current system - already well-tuned
- Inverse-square law prevents node overlap
- Provides outward pressure balancing gravity
- Strength 5000 works well with radius 30 nodes

**Force calculation:**
```javascript
const dx = n2.x - n1.x;
const dy = n2.y - n1.y;
const distanceSq = dx * dx + dy * dy;

if (distanceSq > 0) {
    const force = repulsionStrength / distanceSq;
    const distance = Math.sqrt(distanceSq);
    const fx = (dx / distance) * force;
    const fy = (dy / distance) * force;

    n1.vx -= fx;
    n1.vy -= fy;
    n2.vx += fx;
    n2.vy += fy;
}
```

#### 3. Edge Attraction (Optional)
```javascript
edgeAttractionStrength: 0.02  // Weak attraction
edgeAttractionEnabled: true    // Can be toggled
```

**Rationale:**
- Weak attraction along edges maintains graph structure
- Much weaker than old spring forces (0.02 vs 0.05)
- Helps dependent tasks stay near each other
- Optional - can disable for pure circular layout

**Force calculation:**
```javascript
const dx = toNode.x - fromNode.x;
const dy = toNode.y - fromNode.y;
const force = edgeAttractionStrength;

fromNode.vx += dx * force;
fromNode.vy += dy * force;
toNode.vx -= dx * force;
toNode.vy -= dy * force;
```

### Motion Damping

```javascript
damping: 0.92  // Slightly increased from 0.9
```

**Rationale:**
- Higher damping (0.92 vs 0.9) for faster convergence
- Reduces oscillation around equilibrium
- System settles to stable state more quickly
- Still allows smooth, organic motion

### Velocity Limits (New)

```javascript
maxVelocity: 20  // Prevent extreme speeds
```

**Rationale:**
- Cap maximum velocity to prevent visual artifacts
- Prevents nodes "shooting" across screen
- Maintains smooth, controlled motion
- Applied after all forces calculated

```javascript
const speed = Math.sqrt(node.vx * node.vx + node.vy * node.vy);
if (speed > maxVelocity) {
    node.vx = (node.vx / speed) * maxVelocity;
    node.vy = (node.vy / speed) * maxVelocity;
}
```

## Equilibrium Analysis

### Expected Equilibrium Radius

For N nodes uniformly distributed in a ring:

**Gravity (inward):** F_gravity = k_gravity = 0.08

**Repulsion (outward):** F_repulsion ≈ k_repulsion / (spacing²)

At equilibrium with N=10 nodes at radius r=300:
- Spacing between adjacent nodes: 2πr/N ≈ 188 units
- Repulsion force: 5000 / 188² ≈ 0.14

This gives a stable equilibrium where:
- Gravity ≈ 0.08 (inward)
- Repulsion ≈ 0.08-0.14 (outward, depends on local density)

**Expected equilibrium radius:** 250-400 units from center for 5-20 nodes

### Scaling with Node Count

| Nodes | Expected Radius | Cloud Diameter |
|-------|----------------|----------------|
| 5     | 200-250        | 400-500        |
| 10    | 250-350        | 500-700        |
| 20    | 300-450        | 600-900        |
| 50    | 400-600        | 800-1200       |
| 100   | 500-750        | 1000-1500      |

Note: Radius scales roughly with √N due to repulsion dynamics.

## Implementation Changes

### Add to state.graph

```javascript
physics: {
    damping: 0.92,
    gravityStrength: 0.08,
    gravityCenter: { x: 0, y: 0 },
    repulsionStrength: 5000,
    edgeAttractionStrength: 0.02,
    edgeAttractionEnabled: true,
    maxVelocity: 20
}
```

### Replace applySpringForces

```javascript
function applyCircularGravity(nodes, physics) {
    const draggedNode = state.graph.draggedNode;
    const { gravityCenter, gravityStrength } = physics;

    nodes.forEach(node => {
        // Don't apply forces to dragged nodes
        if (node === draggedNode) return;

        const dx = gravityCenter.x - node.x;
        const dy = gravityCenter.y - node.y;
        const distance = Math.sqrt(dx * dx + dy * dy);

        if (distance > 0) {
            const force = gravityStrength;
            node.vx += (dx / distance) * force;
            node.vy += (dy / distance) * force;
        }
    });
}
```

### Add Edge Attraction (Optional)

```javascript
function applyEdgeAttraction(nodes, edges, physics) {
    if (!physics.edgeAttractionEnabled) return;

    const draggedNode = state.graph.draggedNode;
    const { edgeAttractionStrength } = physics;

    edges.forEach(edge => {
        const fromNode = nodes.find(n => n.id === edge.from);
        const toNode = nodes.find(n => n.id === edge.to);
        if (!fromNode || !toNode) return;

        // Don't apply to dragged nodes
        if (fromNode === draggedNode && toNode === draggedNode) return;

        const dx = toNode.x - fromNode.x;
        const dy = toNode.y - fromNode.y;
        const force = edgeAttractionStrength;

        if (fromNode !== draggedNode) {
            fromNode.vx += dx * force;
            fromNode.vy += dy * force;
        }
        if (toNode !== draggedNode) {
            toNode.vx -= dx * force;
            toNode.vy -= dy * force;
        }
    });
}
```

### Update Animation Loop

```javascript
function animateGraph() {
    // ... canvas setup ...

    // Apply forces (NEW ORDER)
    applyCircularGravity(nodes, physics);      // NEW
    applyRepulsionForces(nodes, physics);      // KEEP
    applyEdgeAttraction(nodes, edges, physics); // NEW (optional)

    updatePositions(nodes, physics, canvas);    // MODIFIED (remove boundaries)

    // ... rendering ...
}
```

### Modified updatePositions

Remove boundary clamping (lines 805-810 in current code):

```javascript
function updatePositions(nodes, physics, canvas) {
    const draggedNode = state.graph.draggedNode;
    const { damping, maxVelocity } = physics;

    nodes.forEach(node => {
        // Freeze dragged node
        if (node === draggedNode) {
            node.vx = 0;
            node.vy = 0;
            return;
        }

        // Apply damping
        node.vx *= damping;
        node.vy *= damping;

        // Cap velocity
        const speed = Math.sqrt(node.vx * node.vx + node.vy * node.vy);
        if (speed > maxVelocity) {
            node.vx = (node.vx / speed) * maxVelocity;
            node.vy = (node.vy / speed) * maxVelocity;
        }

        // Update position (NO BOUNDARY CLAMPING)
        node.x += node.vx;
        node.y += node.vy;
    });
}
```

## Initial Node Placement

### Current: Random Placement
```javascript
x: Math.random() * 800 + 100,
y: Math.random() * 400 + 100,
```

### Proposed: Circular Initial Layout
```javascript
// Place nodes in a ring around center
const angle = (index / totalNodes) * 2 * Math.PI;
const radius = 300;  // Starting radius
x: Math.cos(angle) * radius,
y: Math.sin(angle) * radius,
vx: 0,
vy: 0,
```

**Rationale:**
- Faster convergence (already near equilibrium)
- More predictable initial layout
- Better visual continuity on reload
- Can add small random offset for organic variation

**Alternative:** Tight cluster at center
```javascript
x: (Math.random() - 0.5) * 50,
y: (Math.random() - 0.5) * 50,
```
- Nodes expand from center
- Nice visual effect of cloud formation
- Takes longer to reach equilibrium

**Recommendation:** Use circular initial layout for production, cluster for demo/visualization.

## Tuning Parameters

### Parameter Ranges for Experimentation

```javascript
// Gravity strength
MIN: 0.04  // Very loose cloud, radius ~500-700
DEFAULT: 0.08  // Balanced cloud, radius ~250-350
MAX: 0.15  // Tight cloud, radius ~150-200

// Repulsion strength
MIN: 3000  // Nodes can get closer
DEFAULT: 5000  // Current value
MAX: 8000  // More spacing between nodes

// Edge attraction
DISABLED: 0  // Pure circular layout
LIGHT: 0.01  // Very subtle structure
DEFAULT: 0.02  // Noticeable dependency clusters
STRONG: 0.04  // Approaches old spring behavior

// Damping
MIN: 0.85  // More oscillation, livelier
DEFAULT: 0.92  // Quick settle, smooth
MAX: 0.98  // Very slow movement
```

## Visual Enhancements

### Center Point Indicator (Optional)

Draw a subtle marker at gravity center:
```javascript
ctx.beginPath();
ctx.arc(centerScreen.x, centerScreen.y, 5, 0, Math.PI * 2);
ctx.fillStyle = 'rgba(74, 144, 226, 0.3)';
ctx.fill();
```

### Orbit Radius Guides (Optional)

Draw subtle circles showing equilibrium radii:
```javascript
const radii = [200, 350, 500];
radii.forEach(r => {
    const screenR = r * viewport.zoom;
    ctx.beginPath();
    ctx.arc(centerScreen.x, centerScreen.y, screenR, 0, Math.PI * 2);
    ctx.strokeStyle = 'rgba(74, 144, 226, 0.1)';
    ctx.lineWidth = 1;
    ctx.stroke();
});
```

## Testing Strategy

### Unit Tests
1. Gravity force calculation
2. Equilibrium radius with known node count
3. Velocity clamping
4. Force application to correct nodes (not dragged)

### Integration Tests
1. System reaches stable state (velocities → 0)
2. Nodes don't escape to infinity
3. No node overlap (collision detection)
4. Edge attraction maintains graph structure

### Visual Tests
1. 5 nodes: Small, tight cloud
2. 10 nodes: Balanced circular layout
3. 25 nodes: Larger cloud, clear structure
4. 50+ nodes: Performance, no overlap
5. Star graph (1 center, N leaves): Center stays central
6. Chain graph (linear): Forms curved arc
7. Complete graph (all connected): Forms ring

### Performance Tests
- FPS with 50 nodes
- FPS with 100 nodes
- Force calculation time per frame
- Memory usage over 5 minutes

## Success Criteria

- ✓ Nodes form circular cloud within 3 seconds
- ✓ Cloud radius proportional to √N
- ✓ No nodes escape beyond 2x equilibrium radius
- ✓ No node overlap (min distance > 2 * radius)
- ✓ Graph structure visible (dependencies near each other)
- ✓ Smooth animation at 60 FPS with 50 nodes
- ✓ System stable (no drift over time)

## Future Enhancements

1. **3D Gravity**: Extend to 3D spherical cloud (WebGL)
2. **Multiple Gravity Wells**: Cluster-based layouts
3. **Adaptive Gravity**: Strength based on node importance
4. **Time-based Animation**: Nodes "launched" from center
5. **User-controllable Center**: Drag to reposition gravity center
6. **Gravity Strength Slider**: Real-time tuning in UI
