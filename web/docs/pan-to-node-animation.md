# Pan-to-Node Animation

This document describes the `panToNode` function for smooth animated camera panning in the Binnacle graph viewer.

## Overview

The `panToNode` function provides smooth, animated camera movement to center on a specific world position. It uses ease-in-out-cubic easing and adaptive duration based on distance traveled.

## Usage

```javascript
import { panToNode } from './graph/index.js';

// Basic usage - pan to node at world coordinates (100, 200)
panToNode(100, 200);

// With options - pan and zoom with callback
panToNode(100, 200, {
    targetZoom: 1.5,        // Optional zoom level
    duration: 800,          // Override adaptive duration (ms)
    canvas: canvasElement,  // Canvas for distance calculation
    onComplete: () => {     // Callback when animation completes
        console.log('Animation finished!');
    }
});
```

## API Reference

### `panToNode(targetX, targetY, options)`

Smoothly pan camera to center on a world position with optional zoom.

**Parameters:**
- `targetX` (number): Target world X coordinate
- `targetY` (number): Target world Y coordinate  
- `options` (object, optional):
  - `targetZoom` (number): Optional target zoom level. Default: current zoom
  - `duration` (number): Override duration in ms. Default: adaptive (300-800ms based on distance)
  - `canvas` (HTMLCanvasElement): Canvas element for distance calculation
  - `onComplete` (function): Callback when animation completes

**Returns:** void

### `cancelPanAnimation()`

Cancel any ongoing pan animation.

**Returns:** void

## Animation Characteristics

### Easing
Uses **ease-in-out-cubic** easing curve:
- Slow start (ease in)
- Fast middle
- Slow end (ease out)

This provides a smooth, modern feel to camera movements.

### Adaptive Duration

Duration is automatically calculated based on distance traveled (when canvas is provided):

| Distance (pixels) | Duration (ms) |
|-------------------|---------------|
| < 300             | 300           |
| 300-800           | 500           |
| > 800             | 800           |

This ensures short pans feel snappy while long pans don't feel rushed.

### Zoom Support

The animation can simultaneously pan and zoom, smoothly interpolating both values over the same duration.

## Integration with Auto-Follow

The pan animation integrates with the existing auto-follow system:

- Manual pan/zoom (via mouse) pauses auto-follow
- Programmatic `panToNode` calls can be used by auto-follow without pausing
- Call `resumeAutoFollow()` to resume after manual pause

## Example: Auto-Follow Implementation

```javascript
import { panToNode, getNode } from './graph/index.js';
import * as state from './state.js';

// Listen for tasks transitioning to in_progress
state.subscribe('entities.tasks', (tasks) => {
    const inProgressTask = tasks.find(t => t.status === 'in_progress');
    
    if (inProgressTask && state.get('ui.autoFollow')) {
        const node = getNode(inProgressTask.id);
        
        if (node && node.x !== undefined && node.y !== undefined) {
            panToNode(node.x, node.y, {
                canvas: document.getElementById('graph-canvas'),
                onComplete: () => {
                    state.set('ui.followingNodeId', inProgressTask.id);
                }
            });
        }
    }
});
```

## Example: Search-Driven Camera

```javascript
import { panToNode } from './graph/index.js';

// Zoom to fit multiple search results
function zoomToFitNodes(matchingNodes, canvas) {
    if (matchingNodes.length === 0) return;
    
    // Calculate bounding box
    const bounds = {
        minX: Math.min(...matchingNodes.map(n => n.x)),
        maxX: Math.max(...matchingNodes.map(n => n.x)),
        minY: Math.min(...matchingNodes.map(n => n.y)),
        maxY: Math.max(...matchingNodes.map(n => n.y))
    };
    
    // Calculate center
    const centerX = (bounds.minX + bounds.maxX) / 2;
    const centerY = (bounds.minY + bounds.maxY) / 2;
    
    // Calculate zoom to fit (with padding)
    const boundsWidth = bounds.maxX - bounds.minX;
    const boundsHeight = bounds.maxY - bounds.minY;
    const zoomX = canvas.width / (boundsWidth * 1.2);  // 20% padding
    const zoomY = canvas.height / (boundsHeight * 1.2);
    const targetZoom = Math.min(zoomX, zoomY, 2.0);  // Cap at 2x
    
    // Animate to fit
    panToNode(centerX, centerY, {
        targetZoom,
        canvas
    });
}
```

## Testing

The pan animation includes comprehensive tests in `web/js/graph/transform.test.js`:

- Easing function correctness
- Adaptive duration calculation
- Animation reaches target position
- Animation can be cancelled
- Zoom interpolation works correctly
- Distance-based duration selection

Run tests with:
```bash
node web/js/graph/transform.test.js
```

## Performance

The animation uses `requestAnimationFrame` for smooth 60 FPS rendering. The interpolation calculations are lightweight:

- No memory allocations per frame
- Simple arithmetic operations only
- State updates trigger graph re-render via existing state subscription system

## Browser Compatibility

Requires:
- `requestAnimationFrame` (all modern browsers)
- `cancelAnimationFrame` (all modern browsers)
- `performance.now()` (all modern browsers)

## Related Features

- **Auto-Follow** (`PRD_GUI_CAMERA_FOLLOW.md`) - Uses panToNode to track active tasks
- **Search Camera** (`PRD_SEARCH_CAMERA_FOLLOW.md`) - Uses panToNode to fit search results
- **Camera Controls** (`web/js/graph/camera.js`) - Manual pan/zoom that pauses auto-follow
