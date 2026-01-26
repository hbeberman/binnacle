/**
 * Binnacle Graph - Camera Controls
 * 
 * Handles user interactions for camera control:
 * - Pan (click and drag)
 * - Zoom (mouse wheel or buttons)
 * - Reset view
 * - Hover detection for nodes and edges
 */

import * as state from '../state.js';
import { applyZoom, applyPan, resetViewport, screenToWorld } from './transform.js';
import { findNodeAtPosition, findEdgeAtPosition, setHoveredNode } from './renderer.js';

// Mouse interaction state
let isDragging = false;
let dragStartX = 0;
let dragStartY = 0;

// Hover state
let currentHoveredNode = null;
let currentHoveredEdge = null;

// Callback functions for tooltip handling
let onNodeHover = null;
let onEdgeHover = null;
let onHoverEnd = null;

// Canvas reference
let canvas = null;

/**
 * Initialize camera controls on a canvas element
 * @param {HTMLCanvasElement} canvasElement - The canvas to attach controls to
 * @param {Object} callbacks - Optional callback functions
 * @param {Function} callbacks.onNodeHover - Called when hovering over a node: (node, mouseX, mouseY)
 * @param {Function} callbacks.onEdgeHover - Called when hovering over an edge: (edge, mouseX, mouseY)
 * @param {Function} callbacks.onHoverEnd - Called when hover ends: ()
 */
export function init(canvasElement, callbacks = {}) {
    canvas = canvasElement;
    
    // Store callbacks
    onNodeHover = callbacks.onNodeHover || null;
    onEdgeHover = callbacks.onEdgeHover || null;
    onHoverEnd = callbacks.onHoverEnd || null;
    
    // Mouse events for pan and hover
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
 * Handle mouse move - pan camera if dragging, otherwise check hover
 */
function onMouseMove(e) {
    const rect = canvas.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;
    
    if (isDragging) {
        const dx = e.clientX - dragStartX;
        const dy = e.clientY - dragStartY;
        
        applyPan(dx, dy);
        
        // Pause auto-follow on manual pan
        pauseAutoFollow();
        
        dragStartX = e.clientX;
        dragStartY = e.clientY;
        
        // Hide tooltips while dragging
        if (onHoverEnd) {
            onHoverEnd();
        }
        currentHoveredNode = null;
        currentHoveredEdge = null;
    } else {
        // Hover detection
        checkHover(x, y, e.clientX, e.clientY);
    }
}

/**
 * Check for hover on nodes and edges
 * @param {number} canvasX - X position relative to canvas
 * @param {number} canvasY - Y position relative to canvas
 * @param {number} screenX - X position in screen/window coordinates
 * @param {number} screenY - Y position in screen/window coordinates
 */
function checkHover(canvasX, canvasY, screenX, screenY) {
    // First check for node hover (nodes take priority)
    const hoveredNode = findNodeAtPosition(canvasX, canvasY);
    
    if (hoveredNode) {
        // Hovering over a node
        if (hoveredNode !== currentHoveredNode) {
            currentHoveredNode = hoveredNode;
            currentHoveredEdge = null;
            setHoveredNode(hoveredNode);
            canvas.classList.add('hovering');
            
            if (onNodeHover) {
                onNodeHover(hoveredNode, screenX, screenY);
            }
        } else if (onNodeHover) {
            // Still hovering same node, update position
            onNodeHover(hoveredNode, screenX, screenY);
        }
    } else {
        // Not hovering a node, check for edge
        const hoveredEdge = findEdgeAtPosition(canvasX, canvasY);
        
        if (hoveredEdge) {
            // Hovering over an edge
            if (hoveredEdge !== currentHoveredEdge) {
                currentHoveredEdge = hoveredEdge;
                currentHoveredNode = null;
                setHoveredNode(null);
                canvas.classList.remove('hovering');
                canvas.classList.add('hovering-edge');
                
                if (onEdgeHover) {
                    onEdgeHover(hoveredEdge, screenX, screenY);
                }
            } else if (onEdgeHover) {
                // Still hovering same edge, update position
                onEdgeHover(hoveredEdge, screenX, screenY);
            }
        } else {
            // Not hovering anything
            if (currentHoveredNode || currentHoveredEdge) {
                currentHoveredNode = null;
                currentHoveredEdge = null;
                setHoveredNode(null);
                canvas.classList.remove('hovering');
                canvas.classList.remove('hovering-edge');
                
                if (onHoverEnd) {
                    onHoverEnd();
                }
            }
        }
    }
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
    
    // Clear hover state
    currentHoveredNode = null;
    currentHoveredEdge = null;
    setHoveredNode(null);
    canvas.classList.remove('hovering');
    canvas.classList.remove('hovering-edge');
    
    if (onHoverEnd) {
        onHoverEnd();
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
    
    // Pause auto-follow on manual zoom
    pauseAutoFollow();
}

/**
 * Zoom in by a fixed amount (for button controls)
 */
export function zoomIn() {
    if (!canvas) return;
    applyZoom(1.2, undefined, undefined, canvas);
    pauseAutoFollow();
}

/**
 * Zoom out by a fixed amount (for button controls)
 */
export function zoomOut() {
    if (!canvas) return;
    applyZoom(1 / 1.2, undefined, undefined, canvas);
    pauseAutoFollow();
}

/**
 * Reset camera to default position and zoom
 */
export function resetCamera() {
    resetViewport();
}

/**
 * Pause auto-follow due to user interaction
 * Sets the userPaused flag to prevent auto-follow from moving the camera
 */
function pauseAutoFollow() {
    const autoFollow = state.get('ui.autoFollow');
    const userPaused = state.get('ui.userPaused');
    
    // Only pause if auto-follow is enabled and not already paused
    if (autoFollow && !userPaused) {
        state.set('ui.userPaused', true);
        console.log('Auto-follow paused by user interaction');
    }
}

/**
 * Resume auto-follow after user pause
 * Clears the userPaused flag to allow auto-follow to continue
 */
export function resumeAutoFollow() {
    state.set('ui.userPaused', false);
    console.log('Auto-follow resumed');
}
