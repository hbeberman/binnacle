/**
 * Binnacle Graph - Camera Controls
 * 
 * Handles user interactions for camera control:
 * - Node dragging (click and drag on a node)
 * - Pan (click and drag on empty space, or WASD keys)
 * - Zoom (mouse wheel or buttons)
 * - Reset view
 * - Hover detection for nodes and edges
 * - Touch support with long-press for multi-select
 * - Keyboard shortcuts:
 *   - /: Focus search input (enter search mode)
 *   - Ctrl+A (Cmd+A on Mac): Select all visible nodes
 *   - Escape: Clear selection and focused node
 *   - WASD: Pan the camera
 */

import * as state from '../state.js';
import { toggleSelection, isSelected, selectAll, clearSelection } from '../state.js';
import { applyZoom, applyPan, resetViewport, screenToWorld, getZoom } from './transform.js';
import { findNodeAtPosition, findEdgeAtPosition, setHoveredNode, setDraggedNode, moveNode, getVisibleNodes } from './renderer.js';
import { addCanvasLongPress } from '../utils/touch-handler.js';

// Mouse interaction state
let isDragging = false;
let dragStartX = 0;
let dragStartY = 0;

// Box selection state
let isBoxSelecting = false;
let boxSelectStartX = 0;
let boxSelectStartY = 0;
let boxSelectEndX = 0;
let boxSelectEndY = 0;

// Node dragging state
let draggedNode = null;
let lastDragX = 0;
let lastDragY = 0;
let lastDragTime = 0;
let dragVelocityX = 0;
let dragVelocityY = 0;

// Click detection state
let clickedNodeOnMouseDown = null;
let mouseDownX = 0;
let mouseDownY = 0;
let singleClickTimer = null;
const CLICK_DISTANCE_THRESHOLD = 5; // pixels - max movement to count as click
const DOUBLE_CLICK_DELAY = 300; // ms - delay before single-click callback fires

// Hover state
let currentHoveredNode = null;
let currentHoveredEdge = null;

// Callback functions for tooltip handling
let onNodeHover = null;
let onEdgeHover = null;
let onHoverEnd = null;
let onNodeClick = null;
let onNodeDoubleClick = null;
let onCanvasClick = null;
let onEscape = null;

// Canvas reference
let canvas = null;

// Keyboard panning state
let keysPressedSet = new Set();
let keyboardPanInterval = null;
const PAN_SPEED = 5; // pixels per frame

// Edge auto-pan state (for dragging nodes near screen edge)
let edgeAutoPanInterval = null;
let edgeAutoPanDirection = { x: 0, y: 0 };
const EDGE_PAN_THRESHOLD = 50; // pixels from edge to trigger auto-pan
const EDGE_PAN_SPEED = 8; // pixels per frame
const EDGE_PAN_ACCEL_ZONE = 30; // pixels - zone for acceleration (closer = faster)

/**
 * Initialize camera controls on a canvas element
 * @param {HTMLCanvasElement} canvasElement - The canvas to attach controls to
 * @param {Object} callbacks - Optional callback functions
 * @param {Function} callbacks.onNodeHover - Called when hovering over a node: (node, mouseX, mouseY)
 * @param {Function} callbacks.onEdgeHover - Called when hovering over an edge: (edge, mouseX, mouseY)
 * @param {Function} callbacks.onHoverEnd - Called when hover ends: ()
 * @param {Function} callbacks.onNodeClick - Called when single-clicking a node: (node)
 * @param {Function} callbacks.onNodeDoubleClick - Called when double-clicking a node: (node)
 * @param {Function} callbacks.onCanvasClick - Called when clicking on empty space: ()
 * @param {Function} callbacks.onEscape - Called when Escape key is pressed: ()
 */
export function init(canvasElement, callbacks = {}) {
    canvas = canvasElement;
    
    // Store callbacks
    onNodeHover = callbacks.onNodeHover || null;
    onEdgeHover = callbacks.onEdgeHover || null;
    onHoverEnd = callbacks.onHoverEnd || null;
    onNodeClick = callbacks.onNodeClick || null;
    onNodeDoubleClick = callbacks.onNodeDoubleClick || null;
    onCanvasClick = callbacks.onCanvasClick || null;
    onEscape = callbacks.onEscape || null;
    
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
    
    // Touch support with long-press for multi-select
    addCanvasLongPress(
        canvas,
        (x, y) => findNodeAtPosition(x, y),
        (node, _event) => {
            // Long-press adds to selection without clearing others
            if (node) {
                toggleSelection(node.id, false);
                // Visual feedback - show toast
                state.addToast({
                    type: 'info',
                    message: `Added ${node.id} to selection`,
                    duration: 2000
                });
            }
        }
    );
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
    
    // Track mouse down position for click detection
    mouseDownX = canvasX;
    mouseDownY = canvasY;
    
    // Check if we're clicking on a node
    const clickedNode = findNodeAtPosition(canvasX, canvasY);
    
    if (clickedNode) {
        // Track node for click detection
        clickedNodeOnMouseDown = clickedNode;
        
        // Handle selection based on modifier keys
        const isCtrlOrCmd = e.ctrlKey || e.metaKey;
        const isShift = e.shiftKey;
        
        if (isCtrlOrCmd) {
            // Ctrl/Cmd + click: Toggle selection (add/remove without clearing others)
            toggleSelection(clickedNode.id, false);
        } else if (isShift) {
            // Shift + click: Add to selection (don't remove if already selected)
            if (!isSelected(clickedNode.id)) {
                toggleSelection(clickedNode.id, false);
            }
        } else {
            // Plain click: Select only this node (clear others)
            toggleSelection(clickedNode.id, true);
        }
        
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
        // No node clicked
        clickedNodeOnMouseDown = null;
        
        // Check if Shift is held for box selection
        if (e.shiftKey) {
            // Start box selection
            isBoxSelecting = true;
            boxSelectStartX = canvasX;
            boxSelectStartY = canvasY;
            boxSelectEndX = canvasX;
            boxSelectEndY = canvasY;
            canvas.classList.add('box-selecting');
            
            // Set box selection state for rendering
            state.set('ui.boxSelection', {
                x1: canvasX,
                y1: canvasY,
                x2: canvasX,
                y2: canvasY
            });
        } else {
            // Start dragging the canvas (panning)
            isDragging = true;
            dragStartX = e.clientX;
            dragStartY = e.clientY;
            canvas.classList.add('dragging');
        }
        
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
    
    if (isBoxSelecting) {
        // Update box selection end position
        boxSelectEndX = x;
        boxSelectEndY = y;
        
        // Update box selection state for rendering
        state.set('ui.boxSelection', {
            x1: boxSelectStartX,
            y1: boxSelectStartY,
            x2: boxSelectEndX,
            y2: boxSelectEndY
        });
    } else if (draggedNode) {
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
        
        // Check for edge auto-pan (auto-scroll when dragging near screen edge)
        const panDir = calculateEdgePanDirection(x, y);
        if (panDir.x !== 0 || panDir.y !== 0) {
            startEdgeAutoPan(panDir);
        } else {
            stopEdgeAutoPan();
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
                // Update state for overlay panel
                state.set('ui.hoveredEdge', hoveredEdge);
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
                // Clear edge hover state
                state.set('ui.hoveredEdge', null);
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
    
    if (isBoxSelecting && e.button === 0) {
        // Complete box selection - select all nodes within the box
        const minX = Math.min(boxSelectStartX, boxSelectEndX);
        const maxX = Math.max(boxSelectStartX, boxSelectEndX);
        const minY = Math.min(boxSelectStartY, boxSelectEndY);
        const maxY = Math.max(boxSelectStartY, boxSelectEndY);
        
        // Get all visible nodes
        const visibleNodes = getVisibleNodes();
        
        // Convert screen coordinates to world coordinates for bounds checking
        const topLeft = screenToWorld(minX, minY, canvas);
        const bottomRight = screenToWorld(maxX, maxY, canvas);
        
        // Find nodes within the selection box
        const selectedIds = [];
        for (const node of visibleNodes) {
            // Check if node center is within the box
            if (node.x >= topLeft.x && node.x <= bottomRight.x &&
                node.y >= topLeft.y && node.y <= bottomRight.y) {
                selectedIds.push(node.id);
            }
        }
        
        // Update selection
        if (selectedIds.length > 0) {
            // Replace current selection with box selection
            state.set('graph.selectedNodes', selectedIds);
            
            // Show feedback toast
            state.addToast({
                type: 'info',
                message: `Selected ${selectedIds.length} node${selectedIds.length !== 1 ? 's' : ''}`,
                duration: 2000
            });
        }
        
        // Clear box selection state
        isBoxSelecting = false;
        state.set('ui.boxSelection', null);
        canvas.classList.remove('box-selecting');
    }
    
    // Check for node click (not drag)
    if (e.button === 0 && clickedNodeOnMouseDown && onNodeClick) {
        const rect = canvas.getBoundingClientRect();
        const canvasX = e.clientX - rect.left;
        const canvasY = e.clientY - rect.top;
        
        // Calculate distance moved since mousedown
        const dx = canvasX - mouseDownX;
        const dy = canvasY - mouseDownY;
        const distance = Math.sqrt(dx * dx + dy * dy);
        
        // Only trigger click if mouse didn't move much (not a drag) and no modifiers
        if (distance < CLICK_DISTANCE_THRESHOLD && !e.ctrlKey && !e.metaKey && !e.shiftKey) {
            const nodeToClick = clickedNodeOnMouseDown;
            
            // Cancel any pending single-click timer (for double-click detection)
            if (singleClickTimer) {
                clearTimeout(singleClickTimer);
                singleClickTimer = null;
            }
            
            // Delay single-click callback to allow double-click to take precedence
            singleClickTimer = setTimeout(() => {
                singleClickTimer = null;
                onNodeClick(nodeToClick);
            }, DOUBLE_CLICK_DELAY);
        }
    } else if (e.button === 0 && !clickedNodeOnMouseDown && !isBoxSelecting && onCanvasClick) {
        // Check for canvas click (empty space, not box selection)
        const rect = canvas.getBoundingClientRect();
        const canvasX = e.clientX - rect.left;
        const canvasY = e.clientY - rect.top;
        
        // Calculate distance moved since mousedown
        const dx = canvasX - mouseDownX;
        const dy = canvasY - mouseDownY;
        const distance = Math.sqrt(dx * dx + dy * dy);
        
        // Only trigger canvas click if mouse didn't move much (not a drag/pan)
        if (distance < CLICK_DISTANCE_THRESHOLD) {
            onCanvasClick();
        }
    }
    
    if (draggedNode && e.button === 0) {
        // Stop edge auto-pan
        stopEdgeAutoPan();
        
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
    
    // Reset click tracking
    clickedNodeOnMouseDown = null;
}

/**
 * Handle mouse leave - stop dragging if mouse leaves canvas
 */
function onMouseLeave() {
    if (isBoxSelecting) {
        // Cancel box selection
        isBoxSelecting = false;
        state.set('ui.boxSelection', null);
        canvas.classList.remove('box-selecting');
    }
    
    if (draggedNode) {
        // Stop edge auto-pan
        stopEdgeAutoPan();
        
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
    
    // Cancel single-click timer (double-click takes precedence)
    if (singleClickTimer) {
        clearTimeout(singleClickTimer);
        singleClickTimer = null;
    }
    
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
 * Check if cursor is near canvas edge and calculate auto-pan direction
 * @param {number} x - Cursor X position relative to canvas
 * @param {number} y - Cursor Y position relative to canvas
 * @returns {Object} Pan direction { x: -1/0/1, y: -1/0/1 } and speed multiplier
 */
function calculateEdgePanDirection(x, y) {
    if (!canvas) return { x: 0, y: 0, speedX: 0, speedY: 0 };
    
    const width = canvas.width;
    const height = canvas.height;
    
    let dirX = 0;
    let dirY = 0;
    let speedX = 0;
    let speedY = 0;
    
    // Check left edge
    if (x < EDGE_PAN_THRESHOLD) {
        dirX = 1; // Pan right (moves world left)
        // Accelerate when closer to edge
        const distFromEdge = x;
        speedX = 1 + (1 - Math.min(distFromEdge, EDGE_PAN_ACCEL_ZONE) / EDGE_PAN_ACCEL_ZONE);
    }
    // Check right edge
    else if (x > width - EDGE_PAN_THRESHOLD) {
        dirX = -1; // Pan left (moves world right)
        const distFromEdge = width - x;
        speedX = 1 + (1 - Math.min(distFromEdge, EDGE_PAN_ACCEL_ZONE) / EDGE_PAN_ACCEL_ZONE);
    }
    
    // Check top edge
    if (y < EDGE_PAN_THRESHOLD) {
        dirY = 1; // Pan down (moves world up)
        const distFromEdge = y;
        speedY = 1 + (1 - Math.min(distFromEdge, EDGE_PAN_ACCEL_ZONE) / EDGE_PAN_ACCEL_ZONE);
    }
    // Check bottom edge
    else if (y > height - EDGE_PAN_THRESHOLD) {
        dirY = -1; // Pan up (moves world down)
        const distFromEdge = height - y;
        speedY = 1 + (1 - Math.min(distFromEdge, EDGE_PAN_ACCEL_ZONE) / EDGE_PAN_ACCEL_ZONE);
    }
    
    return { x: dirX, y: dirY, speedX, speedY };
}

/**
 * Start edge auto-pan when dragging a node near screen edge
 * @param {Object} direction - Pan direction from calculateEdgePanDirection
 */
function startEdgeAutoPan(direction) {
    edgeAutoPanDirection = direction;
    
    if (!edgeAutoPanInterval && (direction.x !== 0 || direction.y !== 0)) {
        edgeAutoPanInterval = setInterval(() => {
            // Calculate pan amount based on direction and speed
            const dx = edgeAutoPanDirection.x * EDGE_PAN_SPEED * (edgeAutoPanDirection.speedX || 1);
            const dy = edgeAutoPanDirection.y * EDGE_PAN_SPEED * (edgeAutoPanDirection.speedY || 1);
            
            if (dx !== 0 || dy !== 0) {
                applyPan(dx, dy);
                
                // Also move the dragged node to keep it under the cursor
                // (cursor position in world space changes when we pan)
                if (draggedNode) {
                    // Use last known cursor position
                    const worldPos = screenToWorld(lastDragX, lastDragY, canvas);
                    moveNode(draggedNode, worldPos.x, worldPos.y);
                }
            }
        }, 16); // ~60fps
    }
}

/**
 * Stop edge auto-pan
 */
function stopEdgeAutoPan() {
    if (edgeAutoPanInterval) {
        clearInterval(edgeAutoPanInterval);
        edgeAutoPanInterval = null;
    }
    edgeAutoPanDirection = { x: 0, y: 0, speedX: 0, speedY: 0 };
}

/**
 * Handle keyboard down - WASD panning, Escape to clear, Ctrl+A to select all, / to search
 */
function onKeyDown(e) {
    // Don't handle keyboard if user is typing in an input field
    if (e.target.tagName === 'INPUT' || e.target.tagName === 'TEXTAREA' || e.target.isContentEditable) {
        return;
    }
    
    const key = e.key.toLowerCase();
    
    // / key: Focus search input (enter search mode)
    if (e.key === '/') {
        e.preventDefault();
        const searchInput = document.getElementById('graph-search');
        if (searchInput) {
            searchInput.focus();
            // Select all existing text so user can easily replace it
            searchInput.select();
            console.log('[Keyboard] Entered search mode');
        }
        return;
    }
    
    // Ctrl+A or Cmd+A: Select all visible nodes
    if ((e.ctrlKey || e.metaKey) && key === 'a') {
        e.preventDefault();
        const visibleNodes = getVisibleNodes();
        selectAll(visibleNodes);
        
        // Show feedback toast
        state.addToast({
            type: 'info',
            message: `Selected ${visibleNodes.length} visible node${visibleNodes.length !== 1 ? 's' : ''}`,
            duration: 2000
        });
        
        console.log(`[Keyboard] Selected all ${visibleNodes.length} visible nodes`);
        return;
    }
    
    // Escape key clears selection and focused node
    if (e.key === 'Escape') {
        const selectedNodes = state.getSelectedNodes();
        const focusedNode = state.get('ui.focusedNode');
        
        // Clear selection if any nodes are selected
        if (selectedNodes.length > 0) {
            clearSelection();
            console.log('[Keyboard] Selection cleared by Escape key');
            
            // Show feedback toast
            state.addToast({
                type: 'info',
                message: 'Selection cleared',
                duration: 2000
            });
        }
        
        // Clear focused node if set
        if (focusedNode) {
            state.set('ui.focusedNode', null);
            console.log('[Keyboard] Focus cleared by Escape key');
        }
        
        // Call onEscape callback to allow hiding info panel
        if (onEscape) {
            onEscape();
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
