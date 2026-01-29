/**
 * Family Reveal Utilities
 * 
 * Graph traversal functions for family lineage reveal feature.
 * Supports ancestor resolution and descendant collection.
 */

import { getNode, getEdges } from '../state.js';

/**
 * Find the family root (PRD or milestone) by walking up child_of edges
 * @param {string} nodeId - Starting node ID
 * @returns {string|null} Root node ID (PRD or milestone), or null if not found
 */
export function findFamilyRoot(nodeId) {
    const visited = new Set();
    let current = nodeId;
    let lastMilestone = null;
    const edges = getEdges();
    
    while (current && !visited.has(current)) {
        visited.add(current);
        const node = getNode(current);
        
        if (!node) {
            // Node not found, stop traversal
            break;
        }
        
        // Check if this is a PRD doc
        if (node.type === 'doc' && node.doc_type === 'prd') {
            return current;
        }
        
        // Track milestones as fallback
        if (node.type === 'milestone') {
            lastMilestone = current;
        }
        
        // Find parent via child_of edge
        const parentEdge = edges.find(e => 
            e.source === current && e.edge_type === 'child_of'
        );
        current = parentEdge?.target;
    }
    
    return lastMilestone; // May be null
}
