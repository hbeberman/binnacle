/**
 * Edge Info Panel Component
 * 
 * Displays detailed information about a selected edge with:
 * - Edge type with color indicator
 * - Source and target nodes
 * - Optional reason
 * - Creation timestamp
 */

import * as state from '../state.js';
import { getEdgeStyle } from '../graph/colors.js';
import { createClickableId } from '../utils/clickable-ids.js';

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
 * Create edge info panel HTML structure
 * @returns {HTMLElement} Edge info panel container element
 */
export function createEdgeInfoPanel() {
    const panel = document.createElement('div');
    panel.className = 'edge-info-panel';
    panel.id = 'edge-info-panel';
    
    panel.innerHTML = `
        <div class="edge-info-header">
            <span class="edge-info-title">Edge Details</span>
            <button class="edge-info-close" id="edge-info-close" title="Close">&times;</button>
        </div>
        <div class="edge-info-type" id="edge-info-type">
            <span class="edge-color-indicator" id="edge-info-color"></span>
            <span id="edge-info-type-name"></span>
        </div>
        <div class="edge-info-connection">
            <div class="edge-info-node">
                <span class="edge-info-node-id" id="edge-info-source-id"></span>
                <span class="edge-info-node-title" id="edge-info-source-title"></span>
            </div>
            <div class="edge-info-arrow">↓</div>
            <div class="edge-info-node">
                <span class="edge-info-node-id" id="edge-info-target-id"></span>
                <span class="edge-info-node-title" id="edge-info-target-title"></span>
            </div>
        </div>
        <div id="edge-info-reason-section" class="edge-info-section" style="display: none;">
            <div class="edge-info-section-title">Reason</div>
            <div id="edge-info-reason" class="edge-info-reason"></div>
        </div>
        <div class="edge-info-date" id="edge-info-date" style="display: none;"></div>
    `;
    
    return panel;
}

/**
 * Initialize edge info panel event handlers
 * @param {HTMLElement} panel - The panel element
 * @param {Object} callbacks - Callback functions
 * @param {Function} callbacks.onClose - Called when panel is closed
 */
export function initializeEdgeInfoPanel(panel, callbacks = {}) {
    const closeBtn = panel.querySelector('#edge-info-close');
    
    if (closeBtn) {
        closeBtn.addEventListener('click', () => {
            hideEdgeInfoPanel();
            if (callbacks.onClose) {
                callbacks.onClose();
            }
        });
    }
}

/**
 * Show the edge info panel
 */
export function showEdgeInfoPanel() {
    const panel = document.getElementById('edge-info-panel');
    if (panel) {
        panel.classList.add('visible');
    }
}

/**
 * Hide the edge info panel
 */
export function hideEdgeInfoPanel() {
    const panel = document.getElementById('edge-info-panel');
    if (panel) {
        panel.classList.remove('visible');
    }
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
 * Update edge info panel content
 * @param {HTMLElement} panel - The panel element
 * @param {Object} edge - Edge data with from, to, edge_type, optional reason
 */
export function updateEdgeInfoPanelContent(panel, edge) {
    if (!edge) {
        hideEdgeInfoPanel();
        return;
    }

    // Get edge style for color
    const style = getEdgeStyle(edge.edge_type);
    
    // Update type with color indicator
    const colorIndicator = panel.querySelector('#edge-info-color');
    const typeName = panel.querySelector('#edge-info-type-name');
    
    if (colorIndicator) {
        colorIndicator.style.backgroundColor = style.color;
    }
    if (typeName) {
        typeName.textContent = formatEdgeTypeName(edge.edge_type);
    }

    // Find source and target entities
    const sourceEntity = findEntity(edge.from);
    const targetEntity = findEntity(edge.to);

    // Update connection info
    const sourceId = panel.querySelector('#edge-info-source-id');
    const sourceTitle = panel.querySelector('#edge-info-source-title');
    const targetId = panel.querySelector('#edge-info-target-id');
    const targetTitle = panel.querySelector('#edge-info-target-title');
    
    if (sourceId) {
        sourceId.textContent = '';
        sourceId.appendChild(createClickableId(edge.from));
    }
    if (sourceTitle) {
        sourceTitle.textContent = sourceEntity?.title || sourceEntity?.name || 'Unknown';
    }
    if (targetId) {
        targetId.textContent = '';
        targetId.appendChild(createClickableId(edge.to));
    }
    if (targetTitle) {
        targetTitle.textContent = targetEntity?.title || targetEntity?.name || 'Unknown';
    }

    // Show/hide reason section
    const reasonSection = panel.querySelector('#edge-info-reason-section');
    const reasonEl = panel.querySelector('#edge-info-reason');
    if (edge.reason) {
        if (reasonEl) reasonEl.textContent = edge.reason;
        if (reasonSection) reasonSection.style.display = 'block';
    } else {
        if (reasonSection) reasonSection.style.display = 'none';
    }

    // Show created date if available
    const dateEl = panel.querySelector('#edge-info-date');
    if (edge.created_at && dateEl) {
        const date = new Date(edge.created_at);
        dateEl.textContent = `Created: ${date.toLocaleDateString()} ${date.toLocaleTimeString()}`;
        dateEl.style.display = 'block';
    } else if (dateEl) {
        dateEl.style.display = 'none';
    }

    // Show panel
    showEdgeInfoPanel();
}
