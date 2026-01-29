/**
 * Spatial hash grid for efficient nearest-neighbor queries
 * Reduces physics repulsion from O(n²) to O(n log n)
 */
export class SpatialHash {
    constructor(cellSize = 150) {
        this.cellSize = cellSize;
        this.grid = new Map();
    }

    /**
     * Get grid cell coordinates for a position
     */
    _getCellKey(x, y) {
        const cellX = Math.floor(x / this.cellSize);
        const cellY = Math.floor(y / this.cellSize);
        return `${cellX},${cellY}`;
    }

    /**
     * Clear the grid and insert all nodes
     */
    rebuild(nodes) {
        this.grid.clear();
        
        for (const node of nodes) {
            const key = this._getCellKey(node.x, node.y);
            if (!this.grid.has(key)) {
                this.grid.set(key, []);
            }
            this.grid.get(key).push(node);
        }
    }

    /**
     * Get all nodes within the specified radius of a position
     * Checks neighboring cells to handle edge cases
     */
    getNearby(x, y, radius) {
        const nearby = [];
        const cellX = Math.floor(x / this.cellSize);
        const cellY = Math.floor(y / this.cellSize);
        
        // Check how many cells to search based on radius
        const cellRadius = Math.ceil(radius / this.cellSize);
        
        for (let dx = -cellRadius; dx <= cellRadius; dx++) {
            for (let dy = -cellRadius; dy <= cellRadius; dy++) {
                const key = `${cellX + dx},${cellY + dy}`;
                const cell = this.grid.get(key);
                if (cell) {
                    nearby.push(...cell);
                }
            }
        }
        
        return nearby;
    }

    /**
     * Get nearby nodes for repulsion calculation
     * Only returns nodes within cutoffDistance
     */
    getNearbyForRepulsion(node, cutoffDistance) {
        const candidates = this.getNearby(node.x, node.y, cutoffDistance);
        const nearby = [];
        
        for (const other of candidates) {
            if (other === node) continue;
            
            const dx = other.x - node.x;
            const dy = other.y - node.y;
            const distSq = dx * dx + dy * dy;
            
            // Only include nodes within cutoff distance
            if (distSq < cutoffDistance * cutoffDistance && distSq > 0) {
                nearby.push(other);
            }
        }
        
        return nearby;
    }
}
