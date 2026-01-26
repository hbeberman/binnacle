/**
 * Binnacle Graph - Camera Controls
 * 
 * Handles user interactions for camera control:
 * - Pan (click and drag)
 * - Zoom (mouse wheel or buttons)
 * - Reset view
 */

import * as state from '../state.js';
import { applyZoom, applyPan, resetViewport, screenToWorld } from './transform.js';

// Mouse interaction state
let isDragging = false;
let dragStartX = 0;
let dragStartY = 0;

// Canvas reference
let canvas = null;

/**
 * Initialize camera controls on a canvas element
 * @param {HTMLCanvasElement} canvasElement - The canvas to attach controls to
 */
export function init(canvasElement) {
    canvas = canvasElement;
    
    // Mouse events for pan
    canvas.addEventListener('mousedown', onMouseDown);
    canvas.addEventListener('mousemove', onMouseMove);
    canvas.addEventListener('mouseup', onMouseUp);
    canvas.addEventListener('mouseleave', onMouseLeave);
    
    // Wheel for zoom
    canvas.addEventListener('wheel', onWheel, { passive: false });
}

/**
 * Handle mouse down - start dragging
 */
function onMouseDown(e) {
    if (e.button !== 0) return; // Only left mouse button
    
    isDragging = true;
    dragStartX = e.clientX;
    dragStartY = e.clientY;
    canvas.classList.add('dragging');
}

/**
 * Handle mouse move - pan camera if dragging
 */
function onMouseMove(e) {
    if (!isDragging) return;
    
    const dx = e.clientX - dragStartX;
    const dy = e.clientY - dragStartY;
    
    applyPan(dx, dy);
    
    dragStartX = e.clientX;
    dragStartY = e.clientY;
}

/**
 * Handle mouse up - stop dragging
 */
function onMouseUp(e) {
    if (e.button !== 0) return;
    
    isDragging = false;
    canvas.classList.remove('dragging');
}

/**
 * Handle mouse leave - stop dragging if mouse leaves canvas
 */
function onMouseLeave() {
    if (isDragging) {
        isDragging = false;
        canvas.classList.remove('dragging');
    }
}

/**
 * Handle mouse wheel - zoom in/out centered on cursor
 */
function onWheel(e) {
    e.preventDefault();
    
    const zoomDelta = e.deltaY > 0 ? 0.9 : 1.1;
    
    // Get cursor position on canvas
    const rect = canvas.getBoundingClientRect();
    const centerX = e.clientX - rect.left;
    const centerY = e.clientY - rect.top;
    
    applyZoom(zoomDelta, centerX, centerY, canvas);
}

/**
 * Zoom in by a fixed amount (for button controls)
 */
export function zoomIn() {
    if (!canvas) return;
    applyZoom(1.2, undefined, undefined, canvas);
}

/**
 * Zoom out by a fixed amount (for button controls)
 */
export function zoomOut() {
    if (!canvas) return;
    applyZoom(1 / 1.2, undefined, undefined, canvas);
}

/**
 * Reset camera to default position and zoom
 */
export function resetCamera() {
    resetViewport();
}
