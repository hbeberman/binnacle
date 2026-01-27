/**
 * Node Detail Pane Component
 * 
 * A pinned side panel that shows comprehensive node details.
 * Triggered by double-clicking a node in the graph.
 * Stays open until explicitly closed, allowing inspection while navigating.
 */

import { getNode } from '../state.js';
import { renderMarkdown } from '../utils/markdown.js';
import { createClickableId, makeIdsClickable } from '../utils/clickable-ids.js';

/**
 * Create the node detail pane HTML
 * @returns {HTMLElement} The pane element
 */
export function createNodeDetailPane() {
    const pane = document.createElement('div');
    pane.className = 'node-detail-pane hidden';
    pane.id = 'node-detail-pane';
    
    pane.innerHTML = `
        <div class="node-detail-pane-header">
            <div class="node-detail-pane-title-section">
                <span class="node-detail-pane-type-badge" id="detail-pane-type-badge"></span>
                <span class="node-detail-pane-id" id="detail-pane-id"></span>
            </div>
            <button class="node-detail-pane-close" id="detail-pane-close" title="Close">&times;</button>
        </div>
        <div class="node-detail-pane-content" id="detail-pane-content">
            <div class="node-detail-pane-loading">Select a node to view details</div>
        </div>
    `;
    
    return pane;
}

/**
 * Format a date timestamp for display
 * @param {string} timestamp - ISO timestamp
 * @returns {string} Formatted date
 */
function formatDate(timestamp) {
    if (!timestamp) return 'N/A';
    const date = new Date(timestamp);
    return date.toLocaleDateString() + ' ' + date.toLocaleTimeString();
}

/**
 * Get status badge HTML
 * @param {string} status - Node status
 * @returns {string} HTML for status badge
 */
function getStatusBadgeHTML(status) {
    const statusColors = {
        pending: 'var(--text-secondary)',
        in_progress: 'var(--accent-blue)',
        blocked: 'var(--accent-red)',
        done: 'var(--accent-green)',
        cancelled: 'var(--text-secondary)',
        reopened: 'var(--accent-orange)',
        seed: 'var(--text-secondary)',
        germinating: 'var(--accent-blue)',
        harvested: 'var(--accent-green)',
        composted: 'var(--text-secondary)'
    };
    
    const color = statusColors[status] || 'var(--text-secondary)';
    const displayStatus = status ? status.replace(/_/g, ' ') : 'unknown';
    
    return `<span class="status-badge" style="background: ${color}; padding: 0.25rem 0.5rem; border-radius: 4px; font-size: 0.75rem; color: white; font-weight: 600;">${displayStatus}</span>`;
}

/**
 * Get priority badge HTML
 * @param {number} priority - Priority (0-4)
 * @returns {string} HTML for priority badge
 */
function getPriorityBadgeHTML(priority) {
    const labels = ['Critical', 'High', 'Medium', 'Low', 'Backlog'];
    const colors = [
        'var(--accent-red)',
        'var(--accent-orange)',
        'var(--accent-blue)',
        'var(--accent-green)',
        'var(--text-secondary)'
    ];
    
    const p = priority ?? 2;
    return `<span class="priority-badge" style="background: ${colors[p]}; padding: 0.25rem 0.5rem; border-radius: 4px; font-size: 0.75rem; color: white; font-weight: 600;">P${p}: ${labels[p]}</span>`;
}

/**
 * Escape HTML to prevent XSS
 * @param {string} str - String to escape
 * @returns {string} Escaped string
 */
function escapeHtml(str) {
    if (!str) return '';
    const div = document.createElement('div');
    div.textContent = str;
    return div.innerHTML;
}

/**
 * Render content for different node types
 * @param {Object} node - Node data
 * @returns {string} HTML content
 */
function renderNodeContent(node) {
    let html = '<div class="detail-pane-section">';
    
    // Title
    html += `<h3 class="detail-pane-node-title">${escapeHtml(node.title || node.name || 'Untitled')}</h3>`;
    
    // Badges for tasks/bugs/milestones
    if (node.type === 'task' || node.type === 'bug' || node.type === 'milestone') {
        html += '<div class="detail-pane-badges" style="display: flex; gap: 0.5rem; margin-bottom: 1rem;">';
        html += getStatusBadgeHTML(node.status);
        if (node.priority !== undefined) {
            html += getPriorityBadgeHTML(node.priority);
        }
        if (node.queued) {
            html += '<span class="queued-badge" style="background: var(--accent-yellow); padding: 0.25rem 0.5rem; border-radius: 4px; font-size: 0.75rem; color: black; font-weight: 600;">⏰ Queued</span>';
        }
        html += '</div>';
    } else if (node.type === 'idea') {
        html += '<div class="detail-pane-badges" style="display: flex; gap: 0.5rem; margin-bottom: 1rem;">';
        html += getStatusBadgeHTML(node.status);
        html += '</div>';
    }
    
    // Short name
    if (node.short_name) {
        html += `<div class="detail-pane-field"><strong>Display Name:</strong> ${escapeHtml(node.short_name)}</div>`;
    }
    
    // Description
    if (node.description) {
        html += `<div class="detail-pane-field"><strong>Description:</strong><br>${escapeHtml(node.description)}</div>`;
    }
    
    // Tags
    if (node.tags && node.tags.length > 0) {
        html += '<div class="detail-pane-field"><strong>Tags:</strong> ';
        html += node.tags.map(tag => `<span class="tag-badge" style="background: var(--bg-tertiary); padding: 0.2rem 0.4rem; border-radius: 3px; font-size: 0.8rem; margin-right: 0.25rem;">${escapeHtml(tag)}</span>`).join('');
        html += '</div>';
    }
    
    // Assignee
    if (node.assignee) {
        html += `<div class="detail-pane-field"><strong>Assignee:</strong> ${escapeHtml(node.assignee)}</div>`;
    }
    
    // Timeline
    html += '<div class="detail-pane-field"><strong>Created:</strong> ' + formatDate(node.created_at) + '</div>';
    if (node.updated_at) {
        html += '<div class="detail-pane-field"><strong>Updated:</strong> ' + formatDate(node.updated_at) + '</div>';
    }
    if (node.closed_at) {
        html += '<div class="detail-pane-field"><strong>Closed:</strong> ' + formatDate(node.closed_at) + '</div>';
    }
    
    // Close reason
    if (node.closed_reason) {
        html += `<div class="detail-pane-field"><strong>Close Reason:</strong><br>${escapeHtml(node.closed_reason)}</div>`;
    }
    
    // Test-specific fields
    if (node.type === 'test') {
        if (node.command) {
            html += `<div class="detail-pane-field"><strong>Command:</strong><br><code style="background: var(--bg-tertiary); padding: 0.25rem 0.5rem; border-radius: 3px; display: block; overflow-x: auto;">${escapeHtml(node.command)}</code></div>`;
        }
        if (node.working_dir) {
            html += `<div class="detail-pane-field"><strong>Working Dir:</strong> <code>${escapeHtml(node.working_dir)}</code></div>`;
        }
        if (node.linked_tasks && node.linked_tasks.length > 0) {
            html += '<div class="detail-pane-field"><strong>Linked Tasks:</strong><br>';
            html += node.linked_tasks.map(taskId => `<div class="linked-task">${escapeHtml(taskId)}</div>`).join('');
            html += '</div>';
        }
    }
    
    // Doc-specific fields
    if (node.type === 'doc') {
        if (node.doc_type) {
            html += `<div class="detail-pane-field"><strong>Doc Type:</strong> ${escapeHtml(node.doc_type)}</div>`;
        }
        if (node.content) {
            html += '<div class="detail-pane-field"><strong>Content:</strong><div class="detail-pane-markdown" id="detail-pane-markdown"></div></div>';
        }
    }
    
    // Relationships
    if (node.edges && node.edges.length > 0) {
        html += '<div class="detail-pane-field"><strong>Relationships:</strong><br>';
        node.edges.forEach(edge => {
            html += `<div class="detail-pane-edge" style="margin: 0.25rem 0; padding: 0.25rem; background: var(--bg-tertiary); border-radius: 3px;">`;
            html += `<span class="edge-type" style="font-weight: 600; color: var(--accent-blue);">${escapeHtml(edge.edge_type)}</span> `;
            html += `<span class="edge-target">${escapeHtml(edge.related_id)}</span>`;
            if (edge.related_title) {
                html += ` <span class="edge-title" style="color: var(--text-secondary);">- ${escapeHtml(edge.related_title)}</span>`;
            }
            html += '</div>';
        });
        html += '</div>';
    }
    
    html += '</div>';
    return html;
}

/**
 * Get node type display info
 * @param {string} type - Node type
 * @returns {Object} Display name and color
 */
function getNodeTypeInfo(type) {
    const types = {
        task: { name: 'Task', color: 'var(--accent-blue)' },
        bug: { name: 'Bug', color: 'var(--accent-red)' },
        milestone: { name: 'Milestone', color: 'var(--accent-purple)' },
        idea: { name: 'Idea', color: 'var(--accent-yellow)' },
        test: { name: 'Test', color: 'var(--accent-green)' },
        doc: { name: 'Document', color: 'var(--accent-orange)' },
        queue: { name: 'Queue', color: 'var(--accent-blue)' },
        agent: { name: 'Agent', color: 'var(--accent-purple)' }
    };
    
    return types[type] || { name: type, color: 'var(--text-secondary)' };
}

/**
 * Show the node detail pane for a specific node
 * @param {string} nodeId - The node ID
 */
export function showNodeDetailPane(nodeId) {
    const pane = document.getElementById('node-detail-pane');
    if (!pane) {
        console.error('Node detail pane not found in DOM');
        return;
    }
    
    const node = getNode(nodeId);
    if (!node) {
        console.error(`Node ${nodeId} not found`);
        return;
    }
    
    // Update header
    const typeEl = document.getElementById('detail-pane-type-badge');
    const idEl = document.getElementById('detail-pane-id');
    
    const typeInfo = getNodeTypeInfo(node.type);
    typeEl.textContent = typeInfo.name;
    typeEl.style.background = typeInfo.color;
    
    idEl.textContent = '';
    idEl.appendChild(createClickableId(node.id));
    
    // Update content
    const contentEl = document.getElementById('detail-pane-content');
    contentEl.innerHTML = renderNodeContent(node);
    
    // Make all binnacle IDs clickable
    contentEl.querySelectorAll('.edge-target, .linked-task').forEach(el => {
        makeIdsClickable(el);
    });
    
    // If it's a doc with content, render markdown
    if (node.type === 'doc' && node.content) {
        const markdownEl = document.getElementById('detail-pane-markdown');
        if (markdownEl) {
            renderMarkdown(markdownEl, node.content);
        }
    }
    
    // Show pane
    pane.classList.remove('hidden');
}

/**
 * Hide the node detail pane
 */
export function hideNodeDetailPane() {
    const pane = document.getElementById('node-detail-pane');
    if (pane) {
        pane.classList.add('hidden');
    }
}

/**
 * Check if the pane is currently visible
 * @returns {boolean} True if visible
 */
export function isNodeDetailPaneVisible() {
    const pane = document.getElementById('node-detail-pane');
    return pane && !pane.classList.contains('hidden');
}

/**
 * Initialize the node detail pane with event handlers
 */
export function initNodeDetailPane() {
    const pane = document.getElementById('node-detail-pane');
    if (!pane) {
        console.error('Node detail pane not found in DOM');
        return;
    }
    
    // Close button
    const closeBtn = document.getElementById('detail-pane-close');
    closeBtn.addEventListener('click', hideNodeDetailPane);
    
    // Close on Escape key
    document.addEventListener('keydown', (e) => {
        if (e.key === 'Escape' && isNodeDetailPaneVisible()) {
            hideNodeDetailPane();
        }
    });
}

/**
 * Mount the node detail pane to the DOM
 * @param {HTMLElement|string} target - Target element or selector
 */
export function mountNodeDetailPane(target) {
    const container = typeof target === 'string' 
        ? document.querySelector(target) 
        : target;
    
    if (!container) {
        console.error('Node detail pane target not found');
        return;
    }
    
    const pane = createNodeDetailPane();
    container.appendChild(pane);
    initNodeDetailPane();
}
