/**
 * Binnacle Graph - Camera Controls
 * 
 * Handles user interactions for camera control:
 * - Node dragging (click and drag on a node)
 * - Pan (click and drag on empty space, or WASD keys)
 * - Zoom (mouse wheel or buttons)
 * - Reset view
 * - Hover detection for nodes and edges
 */

import * as state from '../state.js';
import { applyZoom, applyPan, resetViewport, screenToWorld, getZoom } from './transform.js';
import { findNodeAtPosition, findEdgeAtPosition, setHoveredNode, setDraggedNode, moveNode } from './renderer.js';

// Mouse interaction state
let isDragging = false;
let dragStartX = 0;
let dragStartY = 0;

// Node dragging state
let draggedNode = null;
let lastDragX = 0;
let lastDragY = 0;
let lastDragTime = 0;
let dragVelocityX = 0;
let dragVelocityY = 0;

// Hover state
let currentHoveredNode = null;
let currentHoveredEdge = null;

// Callback functions for tooltip handling
let onNodeHover = null;
let onEdgeHover = null;
let onHoverEnd = null;
let onNodeDoubleClick = null;

// Canvas reference
let canvas = null;

// Keyboard panning state
let keysPressedSet = new Set();
let keyboardPanInterval = null;
const PAN_SPEED = 5; // pixels per frame

/**
 * Initialize camera controls on a canvas element
 * @param {HTMLCanvasElement} canvasElement - The canvas to attach controls to
 * @param {Object} callbacks - Optional callback functions
 * @param {Function} callbacks.onNodeHover - Called when hovering over a node: (node, mouseX, mouseY)
 * @param {Function} callbacks.onEdgeHover - Called when hovering over an edge: (edge, mouseX, mouseY)
 * @param {Function} callbacks.onHoverEnd - Called when hover ends: ()
 * @param {Function} callbacks.onNodeDoubleClick - Called when double-clicking a node: (node)
 */
export function init(canvasElement, callbacks = {}) {
    canvas = canvasElement;
    
    // Store callbacks
    onNodeHover = callbacks.onNodeHover || null;
    onEdgeHover = callbacks.onEdgeHover || null;
    onHoverEnd = callbacks.onHoverEnd || null;
    onNodeDoubleClick = callbacks.onNodeDoubleClick || null;
    
    // Mouse events for pan and hover
    canvas.addEventListener('mousedown', onMouseDown);
    canvas.addEventListener('mousemove', onMouseMove);
    canvas.addEventListener('mouseup', onMouseUp);
    canvas.addEventListener('mouseleave', onMouseLeave);
    canvas.addEventListener('dblclick', onDoubleClick);
    
    // Wheel for zoom
    canvas.addEventListener('wheel', onWheel, { passive: false });
    
    // Keyboard events for WASD panning
    document.addEventListener('keydown', onKeyDown);
    document.addEventListener('keyup', onKeyUp);
}

/**
 * Handle mouse down - start dragging node or canvas
 */
function onMouseDown(e) {
    // Middle mouse button (button 1) always pans
    if (e.button === 1) {
        e.preventDefault(); // Prevent default middle-click behavior
        isDragging = true;
        dragStartX = e.clientX;
        dragStartY = e.clientY;
        canvas.classList.add('dragging');
        
        // Clear focus when manually panning
        clearFocus();
        return;
    }
    
    // Left mouse button (button 0) can drag nodes or pan
    if (e.button !== 0) return;
    
    // Get canvas-relative coordinates
    const rect = canvas.getBoundingClientRect();
    const canvasX = e.clientX - rect.left;
    const canvasY = e.clientY - rect.top;
    
    // Check if we're clicking on a node
    const clickedNode = findNodeAtPosition(canvasX, canvasY);
    
    if (clickedNode) {
        // Start dragging the node
        draggedNode = clickedNode;
        setDraggedNode(clickedNode);
        canvas.classList.add('dragging-node');
        
        // Initialize drag velocity tracking
        lastDragX = canvasX;
        lastDragY = canvasY;
        lastDragTime = Date.now();
        dragVelocityX = 0;
        dragVelocityY = 0;
        
        // Clear focus when clicking a different node (focus is set by link clicks)
        const focusedNode = state.get('ui.focusedNode');
        if (focusedNode && focusedNode !== clickedNode.id) {
            clearFocus();
        }
    } else {
        // Start dragging the canvas (panning)
        isDragging = true;
        dragStartX = e.clientX;
        dragStartY = e.clientY;
        canvas.classList.add('dragging');
        
        // Clear focus when clicking on empty space
        clearFocus();
    }
}

/**
 * Handle mouse move - pan camera if dragging, drag node if node selected, otherwise check hover
 */
function onMouseMove(e) {
    const rect = canvas.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;
    
    if (draggedNode) {
        // Dragging a node - move it to follow cursor
        const worldPos = screenToWorld(x, y, canvas);
        moveNode(draggedNode, worldPos.x, worldPos.y);
        
        // Track velocity for momentum (calculate in screen space, then convert)
        const now = Date.now();
        const dt = now - lastDragTime;
        
        if (dt > 0) {
            // Calculate screen-space velocity (pixels per millisecond)
            const screenDx = x - lastDragX;
            const screenDy = y - lastDragY;
            
            // Exponential moving average for smooth velocity
            const alpha = 0.3;
            dragVelocityX = alpha * (screenDx / dt) + (1 - alpha) * dragVelocityX;
            dragVelocityY = alpha * (screenDy / dt) + (1 - alpha) * dragVelocityY;
            
            lastDragX = x;
            lastDragY = y;
            lastDragTime = now;
        }
        
        // Pause auto-follow while dragging
        pauseAutoFollow();
    } else if (isDragging) {
        // Dragging the canvas - pan the view
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
        // Not dragging anything - check for hover
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
 * Handle mouse up - stop dragging node or canvas
 */
function onMouseUp(e) {
    // Handle both left (0) and middle (1) mouse buttons
    if (e.button !== 0 && e.button !== 1) return;
    
    if (draggedNode && e.button === 0) {
        // Apply momentum to the node based on drag velocity
        // Convert screen velocity to world velocity (accounting for zoom)
        const zoom = getZoom();
        const momentumScale = 0.5; // Tune this for desired momentum strength
        
        draggedNode.vx = (dragVelocityX / zoom) * momentumScale;
        draggedNode.vy = (dragVelocityY / zoom) * momentumScale;
        
        // Stop dragging node
        setDraggedNode(null);
        draggedNode = null;
        canvas.classList.remove('dragging-node');
        
        // Reset velocity tracking
        dragVelocityX = 0;
        dragVelocityY = 0;
    }
    
    if (isDragging) {
        // Stop dragging canvas
        isDragging = false;
        canvas.classList.remove('dragging');
    }
}

/**
 * Handle mouse leave - stop dragging if mouse leaves canvas
 */
function onMouseLeave() {
    if (draggedNode) {
        // Apply momentum even when mouse leaves (same as mouseup)
        const zoom = getZoom();
        const momentumScale = 0.5;
        
        draggedNode.vx = (dragVelocityX / zoom) * momentumScale;
        draggedNode.vy = (dragVelocityY / zoom) * momentumScale;
        
        setDraggedNode(null);
        draggedNode = null;
        canvas.classList.remove('dragging-node');
        
        // Reset velocity tracking
        dragVelocityX = 0;
        dragVelocityY = 0;
    }
    
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
 * Handle double click - show detail pane for clicked node
 */
function onDoubleClick(e) {
    if (!onNodeDoubleClick) return;
    
    // Get canvas-relative coordinates
    const rect = canvas.getBoundingClientRect();
    const canvasX = e.clientX - rect.left;
    const canvasY = e.clientY - rect.top;
    
    // Check if we're double-clicking on a node
    const clickedNode = findNodeAtPosition(canvasX, canvasY);
    
    if (clickedNode) {
        onNodeDoubleClick(clickedNode);
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

/**
 * Clear focused node state
 */
function clearFocus() {
    const focusedNode = state.get('ui.focusedNode');
    if (focusedNode) {
        state.set('ui.focusedNode', null);
        console.log('Focus cleared');
    }
}

/**
 * Handle keyboard down - WASD panning and Escape to clear focus
 */
function onKeyDown(e) {
    // Don't handle keyboard if user is typing in an input field
    if (e.target.tagName === 'INPUT' || e.target.tagName === 'TEXTAREA' || e.target.isContentEditable) {
        return;
    }
    
    const key = e.key.toLowerCase();
    
    // Escape key clears focused node
    if (e.key === 'Escape') {
        const focusedNode = state.get('ui.focusedNode');
        if (focusedNode) {
            state.set('ui.focusedNode', null);
            console.log('Focus cleared by Escape key');
        }
        return;
    }
    
    // Check if it's a WASD key
    if (key === 'w' || key === 'a' || key === 's' || key === 'd') {
        e.preventDefault();
        
        // Add key to set
        keysPressedSet.add(key);
        
        // Start panning interval if not already running
        if (!keyboardPanInterval) {
            keyboardPanInterval = setInterval(handleKeyboardPan, 16); // ~60fps
        }
    }
}

/**
 * Handle keyboard up - stop WASD panning
 */
function onKeyUp(e) {
    const key = e.key.toLowerCase();
    
    if (key === 'w' || key === 'a' || key === 's' || key === 'd') {
        // Remove key from set
        keysPressedSet.delete(key);
        
        // Stop panning interval if no keys are pressed
        if (keysPressedSet.size === 0 && keyboardPanInterval) {
            clearInterval(keyboardPanInterval);
            keyboardPanInterval = null;
        }
    }
}

/**
 * Handle continuous keyboard panning based on pressed keys
 */
function handleKeyboardPan() {
    let dx = 0;
    let dy = 0;
    
    // Calculate pan direction based on pressed keys
    if (keysPressedSet.has('w')) dy += PAN_SPEED;  // Up
    if (keysPressedSet.has('s')) dy -= PAN_SPEED;  // Down
    if (keysPressedSet.has('a')) dx += PAN_SPEED;  // Left
    if (keysPressedSet.has('d')) dx -= PAN_SPEED;  // Right
    
    // Apply panning if any direction is active
    if (dx !== 0 || dy !== 0) {
        applyPan(dx, dy);
        pauseAutoFollow();
    }
}
