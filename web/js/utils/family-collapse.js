/**
 * Family Collapse Utility
 * 
 * Handles collapsing family reveals when info pane is closed.
 * Re-applies global filters and fades out nodes that don't pass.
 */

import * as state from '../state.js';
import { clearRevealAnimations } from './reveal-animation.js';
import { startAnimation } from '../graph/renderer.js';

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
    
    // Start animation loop to render the fade-out effect
    if (nodesToFadeOut.size > 0) {
        startAnimation();
    }
    
    // Deactivate family reveal immediately (stops filter bypass)
    // But keep rootId and revealedNodeIds until animation completes
    state.set('ui.familyReveal.active', false);
    
    // Schedule full cleanup after fade-out animation completes
    setTimeout(() => {
        console.log('[FamilyCollapse] Fade-out complete, clearing reveal state');
        state.set('ui.familyReveal', {
            active: false,
            rootId: null,
            revealedNodeIds: new Set(),
            spawnPositions: new Map()
        });
    }, FADE_OUT_DURATION_MS);
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
        if (passes && hideCompleted) {
            // Hide completed tasks/bugs/milestones (status-based)
            if (node.status === 'done' || node.status === 'cancelled') {
                passes = false;
            }
            // For PRD docs: only hide if they have no uncompleted milestones
            // For other docs: hide by default (they don't have status)
            if (node.type === 'doc') {
                if (node.doc_type === 'prd') {
                    // Show PRD if it has active milestones
                    if (!prdHasActiveWork(node)) {
                        passes = false;
                    }
                } else {
                    // Non-PRD docs: hide by default
                    passes = false;
                }
            }
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
 * Check if a PRD doc node has any pending descendants (milestones, tasks, bugs).
 * Returns true if the PRD should be visible (has active work).
 * @param {Object} prdNode - The PRD doc node
 * @returns {boolean} True if PRD has pending descendants
 */
function prdHasActiveWork(prdNode) {
    const edges = state.get('edges') || [];
    const milestones = state.get('entities.milestones') || [];
    const tasks = state.get('entities.tasks') || [];
    const bugs = state.get('entities.bugs') || [];
    
    // Build parent->children map for efficient traversal
    // For child_of: source is child, target is parent (reversed)
    // For documents: source is doc, target is documented entity (not reversed)
    const childrenOf = new Map();
    for (const edge of edges) {
        if (edge.edge_type === 'child_of') {
            // child_of: source is child, target is parent
            // So parent (target) -> children (sources)
            if (!childrenOf.has(edge.target)) {
                childrenOf.set(edge.target, new Set());
            }
            childrenOf.get(edge.target).add(edge.source);
        } else if (edge.edge_type === 'documents') {
            // documents: source is doc, target is documented entity
            // So doc (source) -> documented entities (targets)
            if (!childrenOf.has(edge.source)) {
                childrenOf.set(edge.source, new Set());
            }
            childrenOf.get(edge.source).add(edge.target);
        }
    }
    
    // Build lookup for all entities by ID
    const entityById = new Map();
    for (const m of milestones) entityById.set(m.id, m);
    for (const t of tasks) entityById.set(t.id, t);
    for (const b of bugs) entityById.set(b.id, b);
    
    // BFS to find all descendants and check if any are pending
    const visited = new Set();
    const queue = [prdNode.id];
    
    while (queue.length > 0) {
        const currentId = queue.shift();
        if (visited.has(currentId)) continue;
        visited.add(currentId);
        
        const children = childrenOf.get(currentId);
        if (!children) continue;
        
        for (const childId of children) {
            const entity = entityById.get(childId);
            if (entity) {
                // Check if this entity is pending (not done/cancelled)
                if (entity.status && entity.status !== 'done' && entity.status !== 'cancelled') {
                    return true;
                }
            }
            // Continue traversal even if entity is completed (might have pending children)
            queue.push(childId);
        }
    }
    
    return false;
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
