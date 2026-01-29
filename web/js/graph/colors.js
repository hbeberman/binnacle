/**
 * Binnacle Graph - Color Utilities
 * 
 * Functions for determining node and edge colors based on type and status.
 */

// Node status colors (for tasks)
const TASK_STATUS_COLORS = {
    'pending': '#5bc0de',
    'in_progress': '#f0ad4e',
    'blocked': '#d9534f',
    'done': '#5cb85c'
};

// Bug colors (reddish tones)
const BUG_STATUS_COLORS = {
    'pending': '#e07878',
    'in_progress': '#d95050',
    'blocked': '#b33a3a',
    'done': '#8fbc8f'
};

// Idea colors (white tones for fluffy cloud appearance)
const IDEA_STATUS_COLORS = {
    'pending': '#ffffff',
    'in_progress': '#ffffff',
    'blocked': '#ffffff',
    'done': '#ffffff'
};

// Milestone colors (orange tones)
const MILESTONE_STATUS_COLORS = {
    'pending': '#ff8c00',
    'in_progress': '#ffa500',
    'blocked': '#ff6347',
    'done': '#ffd700'
};

// Agent colors (cyan/electric blue)
const AGENT_STATUS_COLORS = {
    'active': '#00d4ff',
    'idle': '#6bb3c9',
    'stale': '#4a6670'
};

// Doc colors (by doc_type)
const DOC_TYPE_COLORS = {
    'prd': '#9333ea',
    'note': '#e8b84a',
    'handoff': '#e87d4a'
};

// Queue color (teal) - queues don't have status
const QUEUE_COLOR = '#20b2aa';

// Test colors (use task colors by default)
const TEST_STATUS_COLORS = TASK_STATUS_COLORS;

/**
 * Get the color for a node based on its type and status
 * @param {Object} node - Node object with type, status, and optional doc_type
 * @returns {string} CSS color string
 */
export function getNodeColor(node) {
    const { type, status, doc_type } = node;
    
    switch (type) {
        case 'queue':
            return QUEUE_COLOR;
        
        case 'bug':
            return BUG_STATUS_COLORS[status] || '#e07878';
        
        case 'idea':
            return IDEA_STATUS_COLORS[status] || '#ffffff';
        
        case 'milestone':
            return MILESTONE_STATUS_COLORS[status] || '#228b22';
        
        case 'agent':
            return AGENT_STATUS_COLORS[status] || '#00d4ff';
        
        case 'doc':
            return DOC_TYPE_COLORS[doc_type] || '#4a90e2';
        
        case 'test':
            return TEST_STATUS_COLORS[status] || '#5bc0de';
        
        case 'task':
        default:
            return TASK_STATUS_COLORS[status] || '#5bc0de';
    }
}

// Cache for edge styles to avoid expensive getComputedStyle calls
const edgeStyleCache = new Map();

/**
 * Invalidate the edge style cache (call when theme changes)
 */
function invalidateEdgeStyleCache() {
    edgeStyleCache.clear();
}

/**
 * Get the edge style based on edge type using CSS variables
 * @param {string} edgeType - Edge type
 * @returns {Object} Style object with color, dashed, lineWidth, and optional animated
 */
export function getEdgeStyle(edgeType) {
    // Return cached value if available
    if (edgeStyleCache.has(edgeType)) {
        return edgeStyleCache.get(edgeType);
    }
    
    // Cache miss - compute and cache
    const styles = getComputedStyle(document.documentElement);
    let style;
    
    switch (edgeType) {
        case 'depends_on':
        case 'blocks':
            style = {
                color: styles.getPropertyValue('--edge-blocking').trim() || '#e85d5d',
                dashed: false,
                lineWidth: 2
            };
            break;
        
        case 'related_to':
        case 'caused_by':
        case 'duplicates':
        case 'supersedes':
            style = {
                color: styles.getPropertyValue('--edge-informational').trim() || '#7a8fa3',
                dashed: true,
                lineWidth: 1.5
            };
            break;
        
        case 'fixes':
        case 'tests':
            style = {
                color: styles.getPropertyValue('--edge-fixes').trim() || '#5cb85c',
                dashed: false,
                lineWidth: 2
            };
            break;
        
        case 'parent_of':
        case 'child_of':
            style = {
                color: styles.getPropertyValue('--edge-hierarchy').trim() || '#9b6ed8',
                dashed: false,
                lineWidth: 2
            };
            break;
        
        case 'queued':
            style = {
                color: styles.getPropertyValue('--edge-queued').trim() || '#20b2aa',
                dashed: true,
                lineWidth: 2
            };
            break;
        
        case 'working_on':
            style = {
                color: styles.getPropertyValue('--edge-agent').trim() || '#f0c040',
                dashed: true,
                lineWidth: 3,
                animated: true  // Enable marching ants animation
            };
            break;
        
        case 'worked_on':
            style = {
                color: styles.getPropertyValue('--edge-agent-past').trim() || '#6b7a8a',
                dashed: false,
                lineWidth: 2
            };
            break;
        
        case 'pinned':
            style = {
                color: styles.getPropertyValue('--edge-pinned').trim() || '#5cb85c',
                dashed: false,
                lineWidth: 3
            };
            break;
        
        case 'documents':
            style = {
                color: styles.getPropertyValue('--edge-documents').trim() || '#4a90e2',
                dashed: true,
                lineWidth: 2
            };
            break;
        
        case 'impacts':
            style = {
                color: styles.getPropertyValue('--edge-impacts').trim() || '#e85d5d',
                dashed: true,
                lineWidth: 2
            };
            break;
        
        default:
            style = {
                color: styles.getPropertyValue('--edge-default').trim() || '#3a4d66',
                dashed: false,
                lineWidth: 2
            };
            break;
    }
    
    edgeStyleCache.set(edgeType, style);
    return style;
}

// Cache for edge category colors
const edgeCategoryColorCache = new Map();

/**
 * Invalidate the edge category color cache (call when theme changes)
 */
function invalidateEdgeCategoryColorCache() {
    edgeCategoryColorCache.clear();
}

/**
 * Get the color for an edge category (used in filters)
 * @param {string} category - Edge category (blocking, informational, fixes, hierarchy, queued, agent, pinned, documents, impacts)
 * @returns {string} CSS color string
 */
export function getEdgeCategoryColor(category) {
    // Return cached value if available
    if (edgeCategoryColorCache.has(category)) {
        return edgeCategoryColorCache.get(category);
    }
    
    // Cache miss - compute and cache
    const styles = getComputedStyle(document.documentElement);
    let color;
    
    switch (category) {
        case 'blocking':
            color = styles.getPropertyValue('--edge-blocking').trim() || '#e85d5d';
            break;
        case 'informational':
            color = styles.getPropertyValue('--edge-informational').trim() || '#7a8fa3';
            break;
        case 'fixes':
            color = styles.getPropertyValue('--edge-fixes').trim() || '#5cb85c';
            break;
        case 'hierarchy':
            color = styles.getPropertyValue('--edge-hierarchy').trim() || '#9b6ed8';
            break;
        case 'queued':
            color = styles.getPropertyValue('--edge-queued').trim() || '#20b2aa';
            break;
        case 'agent':
            color = styles.getPropertyValue('--edge-agent').trim() || '#f0c040';
            break;
        case 'pinned':
            color = styles.getPropertyValue('--edge-pinned').trim() || '#5cb85c';
            break;
        case 'documents':
            color = styles.getPropertyValue('--edge-documents').trim() || '#4a90e2';
            break;
        case 'impacts':
            color = styles.getPropertyValue('--edge-impacts').trim() || '#e85d5d';
            break;
        default:
            color = styles.getPropertyValue('--edge-default').trim() || '#3a4d66';
            break;
    }
    
    edgeCategoryColorCache.set(category, color);
    return color;
}

// Cache for CSS colors to avoid expensive getComputedStyle calls every frame
let cssColorsCache = null;

/**
 * Invalidate the CSS colors cache (call when theme changes)
 */
export function invalidateCSSColorsCache() {
    cssColorsCache = null;
    invalidateEdgeStyleCache();
    invalidateEdgeCategoryColorCache();
}

/**
 * Get CSS variables from the document root
 * @returns {Object} Object with commonly used color variables
 */
export function getCSSColors() {
    // Return cached value if available
    if (cssColorsCache !== null) {
        return cssColorsCache;
    }
    
    // Cache miss - compute and cache
    const styles = getComputedStyle(document.documentElement);
    cssColorsCache = {
        bgPrimary: styles.getPropertyValue('--bg-primary').trim() || '#1a2332',
        bgSecondary: styles.getPropertyValue('--bg-secondary').trim() || '#243447',
        textPrimary: styles.getPropertyValue('--text-primary').trim() || '#e8edf3',
        textSecondary: styles.getPropertyValue('--text-secondary').trim() || '#b8c5d6',
        accentBlue: styles.getPropertyValue('--accent-blue').trim() || '#4a90e2',
        queueColor: styles.getPropertyValue('--queue-color').trim() || '#20b2aa'
    };
    
    return cssColorsCache;
}

// Export color constants for direct access if needed
export {
    TASK_STATUS_COLORS,
    BUG_STATUS_COLORS,
    IDEA_STATUS_COLORS,
    MILESTONE_STATUS_COLORS,
    AGENT_STATUS_COLORS,
    DOC_TYPE_COLORS,
    QUEUE_COLOR,
    TEST_STATUS_COLORS
};
