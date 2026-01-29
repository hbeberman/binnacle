/**
 * Spawn Position Calculation
 * 
 * Calculates initial positions for newly revealed nodes in family reveal.
 * Places nodes near their parent, offset in edge flow direction with slight randomization.
 */

/**
 * Compute spawn positions for revealed nodes
 * @param {Map<string, number>} depthMap - Map of node ID to depth level
 * @param {Map<string, object>} existingNodes - Map of existing nodes with positions
 * @param {Array} edges - All edges in the graph
 * @returns {Map<string, {x: number, y: number}>} Map of node ID to spawn position
 */
export function computeSpawnPositions(depthMap, existingNodes, edges) {
    const positions = new Map();
    
    
    // Group nodes by depth for wave-based positioning
    const nodesByDepth = new Map();
    for (const [nodeId, depth] of depthMap.entries()) {
        if (!nodesByDepth.has(depth)) {
            nodesByDepth.set(depth, []);
        }
        nodesByDepth.get(depth).push(nodeId);
    }
    
    // Process each depth level
    const sortedDepths = Array.from(nodesByDepth.keys()).sort((a, b) => a - b);
    
    for (const depth of sortedDepths) {
        const nodesAtDepth = nodesByDepth.get(depth);
        
        for (const nodeId of nodesAtDepth) {
            const position = computeNodeSpawnPosition(nodeId, depth, depthMap, existingNodes, edges, positions);
            positions.set(nodeId, position);
        }
    }
    
    return positions;
}

/**
 * Compute spawn position for a single node
 * @param {string} nodeId - Node ID to position
 * @param {number} depth - Depth level of the node
 * @param {Map<string, number>} depthMap - Map of node ID to depth level
 * @param {Map<string, object>} existingNodes - Map of existing nodes with positions
 * @param {Array} edges - All edges in the graph
 * @param {Map<string, {x: number, y: number}>} positions - Positions computed so far
 * @returns {{x: number, y: number}} Spawn position
 */
function computeNodeSpawnPosition(nodeId, depth, depthMap, existingNodes, edges, positions) {
    
    // Find parent (node with child_of edge pointing to it)
    const parentEdge = edges.find(e => e.source === nodeId && e.edge_type === 'child_of');
    
    
    if (!parentEdge) {
        // No parent - this is the root node
        // Check if it already has a position (existing node)
        const existing = existingNodes.get(nodeId);
        if (existing) {
            return { x: existing.x, y: existing.y };
        }
        
        // New root node - place at origin with slight randomization
        const randomOffset = 20;
        const pos = {
            x: (Math.random() - 0.5) * randomOffset,
            y: (Math.random() - 0.5) * randomOffset
        };
        return pos;
    }
    
    // Get parent position
    const parentId = parentEdge.target;
    let parentPos;
    
    // Check if parent already exists
    const existingParent = existingNodes.get(parentId);
    if (existingParent) {
        parentPos = { x: existingParent.x, y: existingParent.y };
    } else {
        // Parent was just revealed - use computed position
        parentPos = positions.get(parentId);
        if (!parentPos) {
            // Parent not yet positioned (shouldn't happen with depth-first processing)
            // Fall back to origin
            parentPos = { x: 0, y: 0 };
        }
    }
    
    // Check if this node already has a position
    const existing = existingNodes.get(nodeId);
    if (existing) {
        return { x: existing.x, y: existing.y };
    }
    
    // Calculate offset direction (downward from parent, as edges flow from child to parent)
    // Use a downward direction (positive Y) as the base direction
    const baseAngle = Math.PI / 2; // 90 degrees = downward
    
    // Find all siblings at the same depth with same parent
    const allSiblingsWithParent = edges.filter(e => 
        e.edge_type === 'child_of' && 
        e.target === parentId
    ).map(e => e.source);
    
    const siblings = allSiblingsWithParent.filter(sibId => {
        const sibDepth = depthMap.get(sibId);
        return sibDepth === depth && sibId !== nodeId;
    });
    
    // Sort all siblings (including this node) for consistent positioning
    const allSiblingsAtDepth = [...siblings, nodeId].sort();
    const siblingIndex = allSiblingsAtDepth.indexOf(nodeId);
    
    const totalSiblings = allSiblingsAtDepth.length;
    
    // Calculate horizontal spread angle
    let angle = baseAngle;
    if (totalSiblings > 1) {
        const spreadAngle = Math.PI / 4; // 45 degree spread (±22.5 degrees from center)
        const angleOffset = ((siblingIndex / (totalSiblings - 1)) - 0.5) * spreadAngle;
        angle = baseAngle + angleOffset;
    }
    
    // Base distance from parent (increases with depth to spread out the tree)
    const baseDistance = 100 + (depth * 20);
    
    // Add slight randomization to prevent exact overlap (very small to maintain direction)
    const randomOffsetAngle = (Math.random() - 0.5) * 0.05; // ±0.025 radians (~±1.4 degrees)
    const randomOffsetDistance = (Math.random() - 0.5) * 10; // ±5 units
    
    const finalAngle = angle + randomOffsetAngle;
    const finalDistance = Math.max(50, baseDistance + randomOffsetDistance); // Ensure minimum distance
    
    
    return {
        x: parentPos.x + Math.cos(finalAngle) * finalDistance,
        y: parentPos.y + Math.sin(finalAngle) * finalDistance
    };
}
