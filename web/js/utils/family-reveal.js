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

/**
 * Collect all descendants from a root node via BFS traversal
 * @param {string} rootId - Root node ID to start traversal from
 * @returns {Set<string>} Set of all descendant node IDs (including root)
 */
export function collectDescendants(rootId) {
    const descendants = new Set();
    const queue = [rootId];
    const edges = getEdges();
    
    while (queue.length > 0) {
        const current = queue.shift();
        
        // Skip if already visited (cycle detection)
        if (descendants.has(current)) {
            continue;
        }
        
        descendants.add(current);
        
        // Find all children (nodes with child_of edge pointing to current)
        const childEdges = edges.filter(e => 
            e.edge_type === 'child_of' && e.target === current
        );
        
        // Add children to queue
        for (const edge of childEdges) {
            if (!descendants.has(edge.source)) {
                queue.push(edge.source);
            }
        }
    }
    
    return descendants;
}
