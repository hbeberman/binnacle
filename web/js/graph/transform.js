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
