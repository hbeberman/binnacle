/**
 * Binnacle Graph - Coordinate Transform Utilities
 * 
 * Functions for converting between world coordinates (graph layout)
 * and screen coordinates (canvas pixels).
 */

import * as state from '../state.js';

/**
 * Convert screen coordinates to world coordinates
 * @param {number} screenX - Screen X coordinate (pixels from canvas left)
 * @param {number} screenY - Screen Y coordinate (pixels from canvas top)
 * @param {HTMLCanvasElement} canvas - Canvas element
 * @returns {{ x: number, y: number }} World coordinates
 */
export function screenToWorld(screenX, screenY, canvas) {
    const viewport = state.get('ui.viewport');
    const { panX, panY, zoom } = viewport;
    
    return {
        x: (screenX - canvas.width / 2) / zoom - panX,
        y: (screenY - canvas.height / 2) / zoom - panY
    };
}

/**
 * Convert world coordinates to screen coordinates
 * @param {number} worldX - World X coordinate
 * @param {number} worldY - World Y coordinate
 * @param {HTMLCanvasElement} canvas - Canvas element
 * @returns {{ x: number, y: number }} Screen coordinates
 */
export function worldToScreen(worldX, worldY, canvas) {
    const viewport = state.get('ui.viewport');
    const { panX, panY, zoom } = viewport;
    
    return {
        x: (worldX + panX) * zoom + canvas.width / 2,
        y: (worldY + panY) * zoom + canvas.height / 2
    };
}

/**
 * Get the current zoom level from state
 * @returns {number} Current zoom level
 */
export function getZoom() {
    return state.get('ui.viewport.zoom');
}

/**
 * Get the current pan offset from state
 * @returns {{ panX: number, panY: number }} Pan offset
 */
export function getPan() {
    const viewport = state.get('ui.viewport');
    return { panX: viewport.panX, panY: viewport.panY };
}

/**
 * Apply zoom change centered on a specific screen position
 * @param {number} zoomDelta - Zoom multiplier (>1 to zoom in, <1 to zoom out)
 * @param {number} [centerX] - Screen X to zoom around (defaults to canvas center)
 * @param {number} [centerY] - Screen Y to zoom around (defaults to canvas center)
 * @param {HTMLCanvasElement} canvas - Canvas element
 */
export function applyZoom(zoomDelta, centerX, centerY, canvas) {
    const viewport = state.get('ui.viewport');
    
    // Default to canvas center
    if (centerX === undefined) centerX = canvas.width / 2;
    if (centerY === undefined) centerY = canvas.height / 2;
    
    // Get world position before zoom
    const worldBefore = screenToWorld(centerX, centerY, canvas);
    
    // Apply zoom with limits
    const newZoom = Math.max(
        viewport.minZoom,
        Math.min(viewport.maxZoom, viewport.zoom * zoomDelta)
    );
    
    // Update zoom
    state.set('ui.viewport.zoom', newZoom);
    
    // Get world position after zoom
    const worldAfter = screenToWorld(centerX, centerY, canvas);
    
    // Adjust pan to keep the same world point under cursor
    const newPanX = viewport.panX + (worldAfter.x - worldBefore.x);
    const newPanY = viewport.panY + (worldAfter.y - worldBefore.y);
    
    state.set('ui.viewport.panX', newPanX);
    state.set('ui.viewport.panY', newPanY);
}

/**
 * Pan the viewport by a delta
 * @param {number} deltaX - X pan delta (in screen pixels)
 * @param {number} deltaY - Y pan delta (in screen pixels)
 */
export function applyPan(deltaX, deltaY) {
    const viewport = state.get('ui.viewport');
    const zoom = viewport.zoom;
    
    // Convert screen delta to world delta
    const worldDeltaX = deltaX / zoom;
    const worldDeltaY = deltaY / zoom;
    
    state.set('ui.viewport.panX', viewport.panX + worldDeltaX);
    state.set('ui.viewport.panY', viewport.panY + worldDeltaY);
}

/**
 * Center the viewport on a specific world position
 * @param {number} worldX - World X coordinate to center on
 * @param {number} worldY - World Y coordinate to center on
 */
export function centerOn(worldX, worldY) {
    // Cancel any ongoing pan animation to avoid conflicts
    cancelPanAnimation();
    
    // To center (worldX, worldY), we need to adjust pan so that:
    // screenX = (worldX + panX) * zoom + canvas.width / 2 = canvas.width / 2
    // This means: (worldX + panX) * zoom = 0
    // So: panX = -worldX
    state.set('ui.viewport.panX', -worldX);
    state.set('ui.viewport.panY', -worldY);
}

/**
 * Reset the viewport to default position and zoom
 */
export function resetViewport() {
    state.set('ui.viewport', {
        panX: 0,
        panY: 0,
        zoom: 1.0,
        minZoom: 0.1,
        maxZoom: 3.0
    });
}

/**
 * Get the visible world bounds for the current viewport
 * @param {HTMLCanvasElement} canvas - Canvas element
 * @returns {{ minX: number, maxX: number, minY: number, maxY: number }} World bounds
 */
export function getVisibleBounds(canvas) {
    const topLeft = screenToWorld(0, 0, canvas);
    const bottomRight = screenToWorld(canvas.width, canvas.height, canvas);
    
    return {
        minX: topLeft.x,
        maxX: bottomRight.x,
        minY: topLeft.y,
        maxY: bottomRight.y
    };
}

/**
 * Check if a world position is within the visible viewport
 * @param {number} worldX - World X coordinate
 * @param {number} worldY - World Y coordinate
 * @param {number} margin - Extra margin (in world units) to include
 * @param {HTMLCanvasElement} canvas - Canvas element
 * @returns {boolean} True if position is visible
 */
export function isInViewport(worldX, worldY, margin, canvas) {
    const bounds = getVisibleBounds(canvas);
    return (
        worldX >= bounds.minX - margin &&
        worldX <= bounds.maxX + margin &&
        worldY >= bounds.minY - margin &&
        worldY <= bounds.maxY + margin
    );
}

// Animation state for pan-to-node
let panAnimation = null;

/**
 * Ease-in-out-cubic timing function
 * @param {number} t - Progress value from 0 to 1
 * @returns {number} Eased progress value
 */
function easeInOutCubic(t) {
    return t < 0.5 
        ? 4 * t * t * t 
        : 1 - Math.pow(-2 * t + 2, 3) / 2;
}

/**
 * Calculate adaptive animation duration based on distance
 * @param {number} distance - Distance in screen pixels
 * @returns {number} Duration in milliseconds
 */
function calculateDuration(distance) {
    if (distance < 300) return 300;
    if (distance < 800) return 500;
    return 800;
}

/**
 * Smoothly pan camera to center on a world position with optional zoom
 * @param {number} targetX - Target world X coordinate
 * @param {number} targetY - Target world Y coordinate
 * @param {Object} options - Animation options
 * @param {number} [options.targetZoom] - Optional target zoom level
 * @param {number} [options.duration] - Override duration in ms (default: adaptive)
 * @param {string} [options.nodeId] - Optional node ID to validate during animation
 * @param {Function} [options.onComplete] - Callback when animation completes
 * @param {Function} [options.onNodeDisappeared] - Callback if node disappears during animation
 * @param {HTMLCanvasElement} [options.canvas] - Canvas element (for distance calculation)
 */
export function panToNode(targetX, targetY, options = {}) {
    // Cancel any existing animation
    if (panAnimation) {
        cancelAnimationFrame(panAnimation.frameId);
    }
    
    const viewport = state.get('ui.viewport');
    const startPanX = viewport.panX;
    const startPanY = viewport.panY;
    const startZoom = viewport.zoom;
    
    const targetPanX = -targetX;
    const targetPanY = -targetY;
    const targetZoom = options.targetZoom !== undefined ? options.targetZoom : startZoom;
    
    // Calculate distance for adaptive duration
    let duration = options.duration;
    if (duration === undefined && options.canvas) {
        const canvas = options.canvas;
        const startScreen = worldToScreen(targetX, targetY, canvas);
        const centerX = canvas.width / 2;
        const centerY = canvas.height / 2;
        const distance = Math.sqrt(
            Math.pow(startScreen.x - centerX, 2) + 
            Math.pow(startScreen.y - centerY, 2)
        );
        duration = calculateDuration(distance);
    } else if (duration === undefined) {
        duration = 500; // Default duration if no canvas provided
    }
    
    const startTime = performance.now();
    
    // Animation state
    panAnimation = {
        frameId: null,
        cancelled: false
    };
    
    function animate(currentTime) {
        if (panAnimation.cancelled) {
            return;
        }
        
        // If nodeId provided, check if node still exists
        if (options.nodeId) {
            const node = state.getNode(options.nodeId);
            if (!node || typeof node.x !== 'number' || typeof node.y !== 'number') {
                // Node disappeared - cancel animation and notify
                console.warn(`[panToNode] Node ${options.nodeId} disappeared during animation`);
                panAnimation = null;
                if (options.onNodeDisappeared) {
                    options.onNodeDisappeared();
                }
                return;
            }
        }
        
        const elapsed = currentTime - startTime;
        const progress = Math.min(elapsed / duration, 1.0);
        const eased = easeInOutCubic(progress);
        
        // Interpolate pan and zoom
        const currentPanX = startPanX + (targetPanX - startPanX) * eased;
        const currentPanY = startPanY + (targetPanY - startPanY) * eased;
        const currentZoom = startZoom + (targetZoom - startZoom) * eased;
        
        // Update viewport
        state.set('ui.viewport.panX', currentPanX);
        state.set('ui.viewport.panY', currentPanY);
        state.set('ui.viewport.zoom', currentZoom);
        
        if (progress < 1.0) {
            panAnimation.frameId = requestAnimationFrame(animate);
        } else {
            // Animation complete
            panAnimation = null;
            if (options.onComplete) {
                options.onComplete();
            }
        }
    }
    
    panAnimation.frameId = requestAnimationFrame(animate);
}

/**
 * Cancel any ongoing pan animation
 */
export function cancelPanAnimation() {
    if (panAnimation) {
        panAnimation.cancelled = true;
        if (panAnimation.frameId) {
            cancelAnimationFrame(panAnimation.frameId);
        }
        panAnimation = null;
    }
}
