/**
 * Graph Overlay Panel Component
 * 
 * Displays compact node/edge information on hover:
 * - Entity ID, title (truncated), type icon, status badge for nodes
 * - Relationship type, source → target for edges
 * - Empty state with compass emoji when nothing hovered
 */

import * as state from '../state.js';
import { createClickableId } from '../utils/clickable-ids.js';
import { getEdgeStyle } from '../graph/colors.js';

/**
 * Get icon for entity type
 * @param {string} type - Entity type
 * @returns {string} Icon emoji
 */
function getEntityIcon(type) {
    const icons = {
        'task': '📋',
        'bug': '🐛',
        'idea': '💡',
        'test': '🧪',
        'doc': '📄',
        'milestone': '🎯',
        'queue': '📥',
        'agent': '🤖'
    };
    return icons[type] || '•';
}

/**
 * Get status badge info
 * @param {Object} entity - Entity with status
 * @returns {Object|null} Badge info {text, class}
 */
function getStatusBadge(entity) {
    if (!entity.status) return null;
    
    const statusMap = {
        'pending': { text: 'Pending', class: 'status-pending' },
        'in_progress': { text: 'In Progress', class: 'status-in-progress' },
        'blocked': { text: 'Blocked', class: 'status-blocked' },
        'done': { text: 'Done', class: 'status-done' },
        'cancelled': { text: 'Cancelled', class: 'status-cancelled' },
        'open': { text: 'Open', class: 'status-open' },
        'closed': { text: 'Closed', class: 'status-closed' },
        'active': { text: 'Active', class: 'status-active' },
        'idle': { text: 'Idle', class: 'status-idle' },
        'stale': { text: 'Stale', class: 'status-stale' }
    };
    
    return statusMap[entity.status] || { text: entity.status, class: 'status-default' };
}

/**
 * Format edge type name for display
 * @param {string} edgeType - Raw edge type
 * @returns {string} Formatted type name
 */
function formatEdgeTypeName(edgeType) {
    const typeNames = {
        'depends_on': 'Depends On',
        'blocks': 'Blocks',
        'child_of': 'Child Of',
        'parent_of': 'Parent Of',
        'related_to': 'Related To',
        'tests': 'Tests',
        'tested_by': 'Tested By',
        'documents': 'Documents',
        'documented_by': 'Documented By',
        'queued': 'Queued',
        'working_on': 'Working On',
        'informational': 'Informational'
    };
    return typeNames[edgeType] || edgeType.replace(/_/g, ' ').replace(/\b\w/g, l => l.toUpperCase());
}

/**
 * Truncate text to max length with ellipsis
 * @param {string} text - Text to truncate
 * @param {number} maxLength - Maximum length
 * @returns {string} Truncated text
 */
function truncateText(text, maxLength = 40) {
    if (!text || text.length <= maxLength) return text || '';
    return text.substring(0, maxLength - 1) + '…';
}

/**
 * Create graph overlay panel HTML structure
 * @returns {HTMLElement} Panel container element
 */
export function createGraphOverlayPanel() {
    const panel = document.createElement('div');
    panel.className = 'graph-overlay-panel';
    panel.id = 'graph-overlay-panel';
    
    panel.innerHTML = `
        <div class="graph-overlay-content" id="graph-overlay-content">
            <div class="graph-overlay-empty" id="graph-overlay-empty">
                <div class="graph-overlay-compass">🧭</div>
            </div>
        </div>
    `;
    
    return panel;
}

/**
 * Find entity by ID from state
 * @param {string} id - Entity ID
 * @returns {Object|null} Entity or null
 */
function findEntity(id) {
    const allEntities = [
        ...state.get('entities.tasks') || [],
        ...state.get('entities.bugs') || [],
        ...state.get('entities.ideas') || [],
        ...state.get('entities.tests') || [],
        ...state.get('entities.docs') || [],
        ...state.get('entities.milestones') || [],
        ...state.get('entities.queues') || [],
        ...state.get('entities.agents') || []
    ];
    
    return allEntities.find(e => e.id === id) || null;
}

/**
 * Update overlay panel with node information
 * @param {HTMLElement} panel - The panel element
 * @param {Object} node - Node data
 */
export function showNodeInfo(panel, node) {
    if (!node) {
        showEmptyState(panel);
        return;
    }

    const content = panel.querySelector('#graph-overlay-content');
    if (!content) return;

    const entity = findEntity(node.id);
    if (!entity) {
        showEmptyState(panel);
        return;
    }

    const icon = getEntityIcon(entity.type);
    const title = truncateText(entity.title || entity.name);
    const statusBadge = getStatusBadge(entity);

    content.innerHTML = `
        <div class="graph-overlay-node-info">
            <div class="graph-overlay-header">
                <span class="graph-overlay-icon">${icon}</span>
                <span class="graph-overlay-id"></span>
            </div>
            <div class="graph-overlay-title">${title}</div>
            ${statusBadge ? `<span class="graph-overlay-status ${statusBadge.class}">${statusBadge.text}</span>` : ''}
        </div>
    `;

    // Add clickable ID
    const idContainer = content.querySelector('.graph-overlay-id');
    if (idContainer) {
        idContainer.appendChild(createClickableId(node.id));
    }
}

/**
 * Update overlay panel with edge information
 * @param {HTMLElement} panel - The panel element
 * @param {Object} edge - Edge data with from, to, edge_type
 */
export function showEdgeInfo(panel, edge) {
    if (!edge) {
        showEmptyState(panel);
        return;
    }

    const content = panel.querySelector('#graph-overlay-content');
    if (!content) return;

    const sourceEntity = findEntity(edge.from);
    const targetEntity = findEntity(edge.to);
    const edgeStyle = getEdgeStyle(edge.edge_type);
    const typeName = formatEdgeTypeName(edge.edge_type);

    const sourceTitle = truncateText(sourceEntity?.title || sourceEntity?.name || 'Unknown', 25);
    const targetTitle = truncateText(targetEntity?.title || targetEntity?.name || 'Unknown', 25);

    content.innerHTML = `
        <div class="graph-overlay-edge-info">
            <div class="graph-overlay-edge-type">
                <span class="graph-overlay-edge-color" style="background-color: ${edgeStyle.color}"></span>
                <span>${typeName}</span>
            </div>
            <div class="graph-overlay-edge-flow">
                <div class="graph-overlay-edge-node">${sourceTitle}</div>
                <div class="graph-overlay-edge-arrow">→</div>
                <div class="graph-overlay-edge-node">${targetTitle}</div>
            </div>
        </div>
    `;
}

/**
 * Show empty state (compass emoji)
 * @param {HTMLElement} panel - The panel element
 */
export function showEmptyState(panel) {
    const content = panel.querySelector('#graph-overlay-content');
    if (!content) return;

    content.innerHTML = `
        <div class="graph-overlay-empty" id="graph-overlay-empty">
            <div class="graph-overlay-compass">🧭</div>
        </div>
    `;
}

/**
 * Mount the graph overlay panel to the DOM
 * @param {string|HTMLElement} target - Target selector or element
 * @returns {HTMLElement} The mounted panel
 */
export function mountGraphOverlayPanel(target) {
    const container = typeof target === 'string' ? document.querySelector(target) : target;
    if (!container) {
        console.error('Graph overlay panel target not found:', target);
        return null;
    }

    const panel = createGraphOverlayPanel();
    container.appendChild(panel);
    
    return panel;
}

/**
 * Initialize graph overlay panel with hover state subscription
 * @param {HTMLElement} panel - The panel element
 */
export function initializeGraphOverlayPanel(panel) {
    if (!panel) return;

    // Subscribe to hovered node state
    state.subscribe('ui.hoveredNode', (nodeId) => {
        if (nodeId) {
            const node = { id: nodeId };
            showNodeInfo(panel, node);
        } else {
            showEmptyState(panel);
        }
    });

    // Subscribe to hovered edge state (if implemented)
    state.subscribe('ui.hoveredEdge', (edge) => {
        if (edge) {
            showEdgeInfo(panel, edge);
        } else {
            showEmptyState(panel);
        }
    });

    // Show initial empty state
    showEmptyState(panel);
}
