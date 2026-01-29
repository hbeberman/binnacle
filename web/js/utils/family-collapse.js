/**
 * Family Collapse Utility
 * 
 * Handles collapsing family reveals when info pane is closed.
 * Re-applies global filters and fades out nodes that don't pass.
 */

import * as state from '../state.js';
import { clearRevealAnimations } from './reveal-animation.js';

// Fade-out animation constants
const FADE_OUT_DURATION_MS = 300;

// Track nodes being faded out: Map<nodeId, startTime>
const fadeOutAnimations = new Map();

/**
 * Collapse active family reveal
 * Re-applies global filters and fades out revealed nodes that don't pass
 */
export function collapseFamilyReveal() {
    const familyReveal = state.get('ui.familyReveal');
    
    // If no active reveal, nothing to do
    if (!familyReveal || !familyReveal.active) {
        return;
    }
    
    console.log('[FamilyCollapse] Collapsing family reveal:', familyReveal.rootId);
    
    // Get revealed nodes before clearing state
    const revealedNodeIds = new Set(familyReveal.revealedNodeIds);
    
    // Clear reveal animations (stops any in-progress fade-ins)
    clearRevealAnimations();
    
    // Determine which revealed nodes should fade out
    // (nodes that don't pass current global filters)
    const nodesToFadeOut = getNodesToFadeOut(revealedNodeIds);
    
    console.log(`[FamilyCollapse] ${nodesToFadeOut.size} nodes will fade out`);
    
    // Start fade-out animations for nodes that will be hidden
    const now = performance.now();
    for (const nodeId of nodesToFadeOut) {
        fadeOutAnimations.set(nodeId, now);
    }
    
    // Clear family reveal state (this will trigger filter update)
    // Nodes that don't pass filters will be hidden after fade-out completes
    state.set('ui.familyReveal', {
        active: false,
        rootId: null,
        revealedNodeIds: new Set(),
        spawnPositions: new Map()
    });
}

/**
 * Determine which revealed nodes should fade out
 * @param {Set<string>} revealedNodeIds - IDs of revealed nodes
 * @returns {Set<string>} IDs of nodes that should fade out
 */
function getNodesToFadeOut(revealedNodeIds) {
    const nodesToFadeOut = new Set();
    
    // Get current filter settings
    const hideCompleted = state.get('ui.hideCompleted');
    const nodeFilters = state.get('ui.nodeTypeFilters') || {};
    const searchQuery = (state.get('ui.searchQuery') || '').toLowerCase().trim();
    
    // Check each revealed node against filters
    for (const nodeId of revealedNodeIds) {
        const node = state.getNode(nodeId);
        if (!node) continue;
        
        // Check if node passes filters
        let passes = true;
        
        // Node type filter
        if (nodeFilters[node.type] === false) {
            passes = false;
        }
        
        // Hide completed filter
        if (passes && hideCompleted && (node.status === 'done' || node.status === 'cancelled')) {
            passes = false;
        }
        
        // Search filter
        if (passes && searchQuery) {
            const matchesId = node.id.toLowerCase().includes(searchQuery);
            const matchesTitle = (node.title || '').toLowerCase().includes(searchQuery);
            const matchesShortName = (node.short_name || '').toLowerCase().includes(searchQuery);
            if (!matchesId && !matchesTitle && !matchesShortName) {
                passes = false;
            }
        }
        
        // If node doesn't pass filters, it should fade out
        if (!passes) {
            nodesToFadeOut.add(nodeId);
        }
    }
    
    return nodesToFadeOut;
}

/**
 * Get current fade-out opacity for a node
 * @param {string} nodeId - Node ID
 * @returns {number|null} Opacity value 0.0-1.0, or null if no active fade-out
 */
export function getFadeOutOpacity(nodeId) {
    const startTime = fadeOutAnimations.get(nodeId);
    if (!startTime) {
        return null; // No active fade-out for this node
    }
    
    const elapsed = performance.now() - startTime;
    if (elapsed >= FADE_OUT_DURATION_MS) {
        // Animation complete, remove from tracking
        fadeOutAnimations.delete(nodeId);
        return 0.0; // Fully transparent (will be filtered out by renderer)
    }
    
    // Ease-out fade out: starts at 1.0, smoothly reaches 0.0
    const progress = elapsed / FADE_OUT_DURATION_MS;
    return 1.0 - easeOutQuad(progress);
}

/**
 * Check if any fade-out animations are active
 * @returns {boolean}
 */
export function hasActiveFadeOutAnimations() {
    return fadeOutAnimations.size > 0;
}

/**
 * Clear all fade-out animations
 */
export function clearFadeOutAnimations() {
    fadeOutAnimations.clear();
}

/**
 * Ease-out quadratic - starts fast, decelerates
 * @param {number} t - Progress 0.0-1.0
 * @returns {number} Eased value
 */
function easeOutQuad(t) {
    return t * (2 - t);
}
