/**
 * Binnacle Tooltip Component
 * 
 * Handles hover tooltips for nodes and edges in the graph canvas.
 * - Node tooltips show: title/short_name, id, status
 * - Edge tooltips show: type, source→target, reason (if any)
 */

import * as state from '../state.js';
import { getEdgeStyle } from '../graph/colors.js';
import { createClickableId } from '../utils/clickable-ids.js';

// DOM element references
let nodeTooltip = null;
let edgeTooltip = null;
let entityTooltip = null;
let container = null;

// Edge type display names
const EDGE_TYPE_NAMES = {
    depends_on: 'Depends On',
    blocks: 'Blocks',
    child_of: 'Child Of',
    parent_of: 'Parent Of',
    fixes: 'Fixes',
    caused_by: 'Caused By',
    documents: 'Documents',
    tests: 'Tests',
    supersedes: 'Supersedes',
    related_to: 'Related To',
    duplicates: 'Duplicates',
    queued: 'Queued',
    pinned: 'Pinned'
};

/**
 * Initialize tooltips by creating DOM elements
 * @param {HTMLElement} containerElement - Container to append tooltips to
 */
export function init(containerElement) {
    container = containerElement;
    
    // Create node tooltip
    nodeTooltip = document.createElement('div');
    nodeTooltip.className = 'graph-tooltip node-tooltip';
    nodeTooltip.innerHTML = `
        <div class="graph-tooltip-title"></div>
        <div class="graph-tooltip-id"></div>
        <div class="graph-tooltip-status"></div>
    `;
    container.appendChild(nodeTooltip);
    
    // Create edge tooltip
    edgeTooltip = document.createElement('div');
    edgeTooltip.className = 'graph-tooltip edge-tooltip';
    edgeTooltip.innerHTML = `
        <div class="edge-tooltip-type"></div>
        <div class="edge-tooltip-ids"></div>
        <div class="edge-tooltip-reason"></div>
    `;
    container.appendChild(edgeTooltip);
    
    // Create entity link tooltip
    entityTooltip = document.createElement('div');
    entityTooltip.className = 'graph-tooltip entity-tooltip';
    entityTooltip.innerHTML = `
        <div class="entity-tooltip-header">
            <span class="entity-tooltip-icon"></span>
            <span class="entity-tooltip-id"></span>
        </div>
        <div class="entity-tooltip-title"></div>
        <div class="entity-tooltip-status"></div>
        <div class="entity-tooltip-description"></div>
    `;
    container.appendChild(entityTooltip);
}

/**
 * Show tooltip for a node
 * @param {Object} node - Node to show tooltip for
 * @param {number} mouseX - Mouse X position (screen coordinates)
 * @param {number} mouseY - Mouse Y position (screen coordinates)
 */
export function showNodeTooltip(node, mouseX, mouseY) {
    if (!nodeTooltip || !node) return;
    
    // Hide edge tooltip when showing node tooltip
    hideEdgeTooltip();
    
    // Update content
    const titleEl = nodeTooltip.querySelector('.graph-tooltip-title');
    const idEl = nodeTooltip.querySelector('.graph-tooltip-id');
    const statusEl = nodeTooltip.querySelector('.graph-tooltip-status');
    
    titleEl.textContent = node.short_name || node.title || node.id;
    idEl.textContent = '';
    idEl.appendChild(createClickableId(node.id));
    
    // Format status with type
    const type = node.type || 'task';
    const status = node.status || 'pending';
    statusEl.textContent = `${type} • ${status}`;
    statusEl.className = `graph-tooltip-status status-${status}`;
    
    // Position and show
    positionTooltip(nodeTooltip, mouseX, mouseY);
    nodeTooltip.classList.add('visible');
    
    // Update state
    state.set('ui.hoveredNode', node.id);
}

/**
 * Hide node tooltip
 */
export function hideNodeTooltip() {
    if (nodeTooltip) {
        nodeTooltip.classList.remove('visible');
    }
    state.set('ui.hoveredNode', null);
}

/**
 * Show tooltip for an edge
 * @param {Object} edge - Edge to show tooltip for
 * @param {number} mouseX - Mouse X position (screen coordinates)
 * @param {number} mouseY - Mouse Y position (screen coordinates)
 */
export function showEdgeTooltip(edge, mouseX, mouseY) {
    if (!edgeTooltip || !edge) return;
    
    // Hide node tooltip when showing edge tooltip
    hideNodeTooltip();
    
    const typeEl = edgeTooltip.querySelector('.edge-tooltip-type');
    const idsEl = edgeTooltip.querySelector('.edge-tooltip-ids');
    const reasonEl = edgeTooltip.querySelector('.edge-tooltip-reason');
    
    // Get edge style for color indicator
    const style = getEdgeStyle(edge.edge_type);
    const typeName = EDGE_TYPE_NAMES[edge.edge_type] || edge.edge_type;
    
    // Type with color dot
    typeEl.innerHTML = `
        <span class="edge-color-dot" style="background-color: ${style.color}; width: 8px; height: 8px; border-radius: 50%; display: inline-block; margin-right: 6px;"></span>
        ${typeName}${edge.bidirectional ? ' ↔' : ''}
    `;
    
    // Source → Target
    idsEl.textContent = `${edge.from || edge.source} → ${edge.to || edge.target}`;
    
    // Reason (if any)
    if (edge.reason) {
        reasonEl.textContent = edge.reason;
        reasonEl.style.display = 'block';
    } else {
        reasonEl.style.display = 'none';
    }
    
    // Position and show
    positionTooltip(edgeTooltip, mouseX, mouseY);
    edgeTooltip.classList.add('visible');
    
    // Update state
    state.set('ui.hoveredEdge', edge);
}

/**
 * Hide edge tooltip
 */
export function hideEdgeTooltip() {
    if (edgeTooltip) {
        edgeTooltip.classList.remove('visible');
    }
    state.set('ui.hoveredEdge', null);
}

/**
 * Hide all tooltips
 */
export function hideAll() {
    hideNodeTooltip();
    hideEdgeTooltip();
}

/**
 * Position tooltip near mouse, adjusting if near viewport edge
 * @param {HTMLElement} tooltip - Tooltip element
 * @param {number} mouseX - Mouse X position
 * @param {number} mouseY - Mouse Y position
 */
function positionTooltip(tooltip, mouseX, mouseY) {
    const offset = 15;
    
    // Initial position
    tooltip.style.left = `${mouseX + offset}px`;
    tooltip.style.top = `${mouseY + offset}px`;
    
    // Force layout to get dimensions
    const rect = tooltip.getBoundingClientRect();
    const viewportWidth = window.innerWidth;
    const viewportHeight = window.innerHeight;
    
    // Adjust if tooltip goes off right edge
    if (rect.right > viewportWidth) {
        tooltip.style.left = `${mouseX - rect.width - offset}px`;
    }
    
    // Adjust if tooltip goes off bottom edge
    if (rect.bottom > viewportHeight) {
        tooltip.style.top = `${mouseY - rect.height - offset}px`;
    }
}

/**
 * Find entity by ID in state
 * @param {string} id - Entity ID
 * @returns {Object|null} Entity or null
 */
function findEntity(id) {
    const entityTypes = ['tasks', 'bugs', 'ideas', 'tests', 'docs', 'milestones', 'queues', 'agents'];
    
    for (const type of entityTypes) {
        const entities = state.get(`entities.${type}`) || [];
        const entity = entities.find(e => e.id === id);
        if (entity) return entity;
    }
    
    return null;
}

/**
 * Update tooltip position (for when hovering same element but mouse moves)
 * @param {string} type - 'node' or 'edge'
 * @param {number} mouseX - Mouse X position
 * @param {number} mouseY - Mouse Y position
 */
export function updatePosition(type, mouseX, mouseY) {
    const tooltip = type === 'node' ? nodeTooltip : edgeTooltip;
    if (tooltip && tooltip.classList.contains('visible')) {
        positionTooltip(tooltip, mouseX, mouseY);
    }
}

/**
 * Show tooltip for an entity link (from markdown, info panels, etc.)
 * @param {string} entityId - Entity ID to show tooltip for
 * @param {number} mouseX - Mouse X position (screen coordinates)
 * @param {number} mouseY - Mouse Y position (screen coordinates)
 */
export function showEntityTooltip(entityId, mouseX, mouseY) {
    if (!entityTooltip || !entityId) return;
    
    // Find entity in state
    const entity = findEntity(entityId);
    if (!entity) {
        console.warn(`Entity ${entityId} not found in state`);
        return;
    }
    
    // Hide other tooltips
    hideNodeTooltip();
    hideEdgeTooltip();
    
    // Get DOM elements
    const iconEl = entityTooltip.querySelector('.entity-tooltip-icon');
    const idEl = entityTooltip.querySelector('.entity-tooltip-id');
    const titleEl = entityTooltip.querySelector('.entity-tooltip-title');
    const statusEl = entityTooltip.querySelector('.entity-tooltip-status');
    const descEl = entityTooltip.querySelector('.entity-tooltip-description');
    
    // Set type icon
    const typeIcons = {
        task: '📋',
        bug: '🐛',
        idea: '💡',
        test: '🧪',
        doc: '📄',
        milestone: '🎯',
        queue: '📥',
        agent: '🤖'
    };
    iconEl.textContent = typeIcons[entity.type] || '📌';
    
    // Set ID
    idEl.textContent = entity.id;
    
    // Set title
    const title = entity.short_name || entity.title || entity.name || entity.id;
    titleEl.textContent = title;
    
    // Set status badge
    const status = entity.status || 'unknown';
    statusEl.textContent = `${entity.type} • ${status}`;
    statusEl.className = `entity-tooltip-status status-${status}`;
    
    // Set description (truncate to ~50 chars)
    if (entity.description) {
        const maxLen = 50;
        const desc = entity.description.length > maxLen 
            ? entity.description.substring(0, maxLen) + '...'
            : entity.description;
        descEl.textContent = desc;
        descEl.style.display = 'block';
    } else {
        descEl.style.display = 'none';
    }
    
    // Position and show
    positionTooltip(entityTooltip, mouseX, mouseY);
    entityTooltip.classList.add('visible');
}

/**
 * Hide entity tooltip
 */
export function hideEntityTooltip() {
    if (entityTooltip) {
        entityTooltip.classList.remove('visible');
    }
}

/**
 * Setup entity link hover listeners
 * Call this after DOM content with entity links is rendered
 * @param {HTMLElement} container - Container element with entity links
 */
export function setupEntityLinkHover(container) {
    if (!container) return;
    
    // Add event delegation for entity links
    container.addEventListener('mouseover', (e) => {
        const entityLink = e.target.closest('.clickable-entity-id');
        if (!entityLink) return;
        
        const entityId = entityLink.dataset.entityId;
        if (entityId) {
            showEntityTooltip(entityId, e.clientX, e.clientY);
        }
    });
    
    container.addEventListener('mouseout', (e) => {
        const entityLink = e.target.closest('.clickable-entity-id');
        if (!entityLink) return;
        
        hideEntityTooltip();
    });
    
    container.addEventListener('mousemove', (e) => {
        const entityLink = e.target.closest('.clickable-entity-id');
        if (!entityLink || !entityTooltip?.classList.contains('visible')) return;
        
        // Update tooltip position as mouse moves
        positionTooltip(entityTooltip, e.clientX, e.clientY);
    });
}
