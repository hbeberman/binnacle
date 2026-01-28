/**
 * Event Queue Processor
 * 
 * Processes events from the eventQueue, panning to new nodes
 * and lingering for 5 seconds before returning to the previous position.
 */

import * as state from '../state.js';
import { panToNode } from './transform.js';

// Constants
const LINGER_DURATION_MS = 5000; // 5 seconds

// Processing state
let processingEvent = false;
let lingerTimeout = null;

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
    
    // Get the node to pan to
    const node = state.getNode(event.entityId);
    if (!node || typeof node.x !== 'number' || typeof node.y !== 'number') {
        console.warn(`[EventProcessor] Node ${event.entityId} not found or has no position, skipping`);
        finishProcessingEvent();
        return;
    }
    
    // Save current camera position and follow state
    const viewport = state.get('ui.viewport');
    const followingNodeId = state.get('ui.followingNodeId');
    
    const savedState = {
        panX: viewport.panX,
        panY: viewport.panY,
        zoom: viewport.zoom,
        followTarget: followingNodeId
    };
    
    console.log(`[EventProcessor] Saved position: panX=${savedState.panX}, panY=${savedState.panY}, zoom=${savedState.zoom}, following=${savedState.followTarget}`);
    
    // Update linger state
    state.set('ui.eventLinger', {
        active: true,
        entityId: event.entityId,
        startTime: Date.now(),
        savedPosition: {
            panX: savedState.panX,
            panY: savedState.panY,
            zoom: savedState.zoom
        },
        savedFollowTarget: savedState.followTarget
    });
    
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
            
            // Schedule return after linger duration
            lingerTimeout = setTimeout(() => {
                returnToPreviousPosition(savedState);
            }, LINGER_DURATION_MS);
        }
    });
}

/**
 * Return camera to previous position after lingering
 * @param {Object} savedState - Saved camera and follow state
 */
function returnToPreviousPosition(savedState) {
    console.log(`[EventProcessor] Linger complete, returning to previous position`);
    
    // If we were following an agent, resume following
    if (savedState.followTarget) {
        const followTarget = state.getNode(savedState.followTarget);
        
        if (followTarget && followTarget.x !== undefined && followTarget.y !== undefined) {
            console.log(`[EventProcessor] Resuming follow of ${savedState.followTarget}`);
            
            panToNode(followTarget.x, followTarget.y, {
                canvas: document.querySelector('#graph-canvas'),
                duration: 500,
                onComplete: () => {
                    // Restore follow state
                    state.set('ui.followingNodeId', savedState.followTarget);
                    finishProcessingEvent();
                }
            });
        } else {
            // Follow target is gone, just return to saved position
            returnToSavedCamera(savedState);
        }
    } else {
        // No follow target, return to saved camera position
        returnToSavedCamera(savedState);
    }
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
            finishProcessingEvent();
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
    
    console.log(`[EventProcessor] Event processing complete. ${updatedQueue.length} events remaining in queue`);
    
    // Process next event if any
    if (updatedQueue.length > 0) {
        // Use setTimeout to avoid recursion and allow state to settle
        setTimeout(() => {
            onEventQueueChanged(updatedQueue);
        }, 100);
    }
}
