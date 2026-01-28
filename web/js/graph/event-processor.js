/**
 * Event Queue Processor
 * 
 * Processes events from the eventQueue, panning to new nodes sequentially
 * and lingering for 5 seconds on each. After the queue is empty, returns to
 * the followed agent (if Follow Agents ON) or stays at the last event position.
 */

import * as state from '../state.js';
import { panToNode } from './transform.js';

// Constants
const LINGER_DURATION_MS = 5000; // 5 seconds

// Processing state
let processingEvent = false;
let lingerTimeout = null;
let savedInitialState = null; // Saved state from before queue processing started

/**
 * Initialize the event processor
 * Subscribes to eventQueue changes and processes events sequentially
 */
export function init() {
    // Subscribe to event queue changes
    state.subscribe('ui.eventQueue', onEventQueueChanged);
    
    console.log('[EventProcessor] Initialized');
}

/**
 * Handle changes to the event queue
 * @param {Array} eventQueue - Current event queue
 */
function onEventQueueChanged(eventQueue) {
    // If already processing an event, wait for it to complete
    if (processingEvent) {
        console.log('[EventProcessor] Already processing event, waiting...');
        return;
    }
    
    // If queue is empty, nothing to do
    if (!eventQueue || eventQueue.length === 0) {
        return;
    }
    
    // Process the first event in the queue
    const event = eventQueue[0];
    processEvent(event);
}

/**
 * Process a single event from the queue
 * @param {Object} event - Event object {entityType, entityId, timestamp}
 */
function processEvent(event) {
    console.log(`[EventProcessor] Processing event: ${event.entityType} ${event.entityId}`);
    
    processingEvent = true;
    state.set('ui.eventPanActive', true);
    
    // Get the node to pan to
    const node = state.getNode(event.entityId);
    if (!node || typeof node.x !== 'number' || typeof node.y !== 'number') {
        console.warn(`[EventProcessor] Node ${event.entityId} not found or has no position, skipping`);
        finishProcessingEvent();
        return;
    }
    
    // Save initial state only once at the start of queue processing
    if (savedInitialState === null) {
        const viewport = state.get('ui.viewport');
        const followingNodeId = state.get('ui.followingNodeId');
        
        savedInitialState = {
            panX: viewport.panX,
            panY: viewport.panY,
            zoom: viewport.zoom,
            followTarget: followingNodeId
        };
        
        console.log(`[EventProcessor] Saved initial state: panX=${savedInitialState.panX}, panY=${savedInitialState.panY}, zoom=${savedInitialState.zoom}, following=${savedInitialState.followTarget}`);
    }
    
    // Update linger state
    state.set('ui.eventLinger', {
        active: true,
        entityId: event.entityId,
        startTime: Date.now(),
        savedPosition: savedInitialState ? {
            panX: savedInitialState.panX,
            panY: savedInitialState.panY,
            zoom: savedInitialState.zoom
        } : null,
        savedFollowTarget: savedInitialState ? savedInitialState.followTarget : null
    });
    
    // Add NEW badge for this entity
    const newBadges = state.get('ui.newBadges') || new Map();
    newBadges.set(event.entityId, Date.now());
    state.set('ui.newBadges', newBadges);
    
    // Pan to the event node
    panToNode(node.x, node.y, {
        canvas: document.querySelector('#graph-canvas'),
        duration: 500,
        targetZoom: 1.5,
        onComplete: () => {
            console.log(`[EventProcessor] Panned to ${event.entityId}, starting ${LINGER_DURATION_MS}ms linger`);
            
            // Select the node to show info panel
            // Note: Setting selectedNode triggers info panel update via state subscriptions
            state.set('ui.selectedNode', event.entityId);
            state.set('ui.selectedNodes', [event.entityId]);
            
            // Schedule completion after linger duration
            // Don't return to previous position yet - wait until queue is empty
            lingerTimeout = setTimeout(() => {
                finishProcessingEvent();
            }, LINGER_DURATION_MS);
        }
    });
}

/**
 * Return camera to saved position
 * @param {Object} savedState - Saved camera state
 */
function returnToSavedCamera(savedState) {
    // Calculate target world coordinates from saved pan values
    // savedState.panX and panY are already in world coordinates (negated)
    const targetX = -savedState.panX;
    const targetY = -savedState.panY;
    
    console.log(`[EventProcessor] Returning to saved position: x=${targetX}, y=${targetY}, zoom=${savedState.zoom}`);
    
    panToNode(targetX, targetY, {
        canvas: document.querySelector('#graph-canvas'),
        duration: 500,
        targetZoom: savedState.zoom,
        onComplete: () => {
            console.log(`[EventProcessor] Returned to saved position`);
        }
    });
}

/**
 * Finish processing current event and move to next
 */
function finishProcessingEvent() {
    // Clear linger state
    state.set('ui.eventLinger', {
        active: false,
        entityId: null,
        startTime: null,
        savedPosition: null,
        savedFollowTarget: null
    });
    
    // Clear any pending linger timeout
    if (lingerTimeout) {
        clearTimeout(lingerTimeout);
        lingerTimeout = null;
    }
    
    // Remove processed event from queue
    const eventQueue = state.get('ui.eventQueue') || [];
    const updatedQueue = eventQueue.slice(1); // Remove first element
    state.set('ui.eventQueue', updatedQueue);
    
    // Mark as no longer processing
    processingEvent = false;
    state.set('ui.eventPanActive', false);
    
    console.log(`[EventProcessor] Event processing complete. ${updatedQueue.length} events remaining in queue`);
    
    // If queue is now empty, return to initial saved state
    if (updatedQueue.length === 0 && savedInitialState !== null) {
        console.log(`[EventProcessor] Queue empty, returning to initial position`);
        returnToInitialPosition();
        return;
    }
    
    // Process next event if any
    if (updatedQueue.length > 0) {
        // Use setTimeout to avoid recursion and allow state to settle
        setTimeout(() => {
            onEventQueueChanged(updatedQueue);
        }, 100);
    }
}

/**
 * Return camera to initial position when queue is empty
 */
function returnToInitialPosition() {
    if (!savedInitialState) {
        console.warn('[EventProcessor] No saved initial state to return to');
        return;
    }
    
    const initialState = savedInitialState;
    savedInitialState = null; // Clear saved state
    
    console.log(`[EventProcessor] Returning to initial position/follow target`);
    
    // If we were following an agent, resume following
    if (initialState.followTarget) {
        const followTarget = state.getNode(initialState.followTarget);
        
        if (followTarget && followTarget.x !== undefined && followTarget.y !== undefined) {
            console.log(`[EventProcessor] Resuming follow of ${initialState.followTarget}`);
            
            panToNode(followTarget.x, followTarget.y, {
                canvas: document.querySelector('#graph-canvas'),
                duration: 500,
                onComplete: () => {
                    // Restore follow state
                    state.set('ui.followingNodeId', initialState.followTarget);
                }
            });
        } else {
            // Follow target is gone, just return to saved position
            returnToSavedCamera(initialState);
        }
    } else {
        // No follow target, stay at last event position
        console.log(`[EventProcessor] No follow target, staying at current position`);
    }
}
