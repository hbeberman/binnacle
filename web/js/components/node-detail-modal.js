/**
 * Node Detail Modal Component
 * 
 * Full-screen modal for viewing comprehensive node details with:
 * - Header with node ID, type badge, and close button
 * - For PRDs/docs: Rendered markdown content
 * - For tasks/bugs/milestones: Extended information (status, priority, dependencies, etc.)
 * - For tests: Test command, working directory, linked tasks
 * - For ideas: Seed information and status
 */

import { getNode } from '../state.js';
import { renderMarkdown } from '../utils/markdown.js';
import { createClickableId, makeIdsClickable } from '../utils/clickable-ids.js';

/**
 * Create the node detail modal HTML
 * @returns {HTMLElement} The modal overlay element
 */
export function createNodeDetailModal() {
    const overlay = document.createElement('div');
    overlay.className = 'node-detail-modal-overlay hidden';
    overlay.id = 'node-detail-modal';
    
    overlay.innerHTML = `
        <div class="node-detail-modal">
            <div class="node-detail-modal-header">
                <div class="node-detail-modal-title-section">
                    <h2 class="node-detail-modal-title" id="node-detail-modal-title">Node Details</h2>
                    <span class="node-detail-modal-type-badge" id="node-detail-modal-type-badge"></span>
                    <span class="node-detail-modal-id" id="node-detail-modal-id"></span>
                </div>
                <button class="node-detail-modal-close" id="node-detail-modal-close" title="Close">&times;</button>
            </div>
            <div class="node-detail-modal-content" id="node-detail-modal-content">
                <div class="node-detail-modal-loading">Loading...</div>
            </div>
        </div>
    `;
    
    return overlay;
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
    
    return `<span class="status-badge" style="background: ${color};">${displayStatus}</span>`;
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
    return `<span class="priority-badge" style="background: ${colors[p]};">P${p}: ${labels[p]}</span>`;
}

/**
 * Render content for a doc node
 * @param {Object} node - Doc node
 * @returns {string} HTML content
 */
function renderDocContent(node) {
    const metaHTML = `
        <div class="node-detail-section">
            <h3 class="node-detail-section-title">Metadata</h3>
            <div class="node-detail-meta-grid">
                <div class="node-detail-meta-item">
                    <span class="node-detail-meta-label">Type:</span>
                    <span class="node-detail-meta-value">${node.doc_type || 'note'}</span>
                </div>
                <div class="node-detail-meta-item">
                    <span class="node-detail-meta-label">Created:</span>
                    <span class="node-detail-meta-value">${formatDate(node.created_at)}</span>
                </div>
                ${node.updated_at ? `
                    <div class="node-detail-meta-item">
                        <span class="node-detail-meta-label">Updated:</span>
                        <span class="node-detail-meta-value">${formatDate(node.updated_at)}</span>
                    </div>
                ` : ''}
                ${node.version ? `
                    <div class="node-detail-meta-item">
                        <span class="node-detail-meta-label">Version:</span>
                        <span class="node-detail-meta-value">${node.version}</span>
                    </div>
                ` : ''}
            </div>
        </div>
    `;
    
    const contentHTML = node.content ? `
        <div class="node-detail-section">
            <h3 class="node-detail-section-title">Content</h3>
            <div class="node-detail-markdown-content" id="node-detail-markdown-content"></div>
        </div>
    ` : `
        <div class="node-detail-section">
            <p class="node-detail-empty">This document has no content yet.</p>
        </div>
    `;
    
    return metaHTML + contentHTML;
}

/**
 * Render content for a task/bug/milestone node
 * @param {Object} node - Task/bug/milestone node
 * @returns {string} HTML content
 */
function renderTaskContent(node) {
    const html = `
        <div class="node-detail-section">
            <div class="node-detail-badges">
                ${getStatusBadgeHTML(node.status, node.type)}
                ${(node.priority !== undefined) ? getPriorityBadgeHTML(node.priority) : ''}
                ${node.queued ? '<span class="queued-badge">⏰ Queued</span>' : ''}
            </div>
        </div>
        
        ${node.short_name ? `
            <div class="node-detail-section">
                <h3 class="node-detail-section-title">Display Name</h3>
                <p class="node-detail-short-name">${escapeHtml(node.short_name)}</p>
            </div>
        ` : ''}
        
        ${node.description ? `
            <div class="node-detail-section">
                <h3 class="node-detail-section-title">Description</h3>
                <p class="node-detail-description">${escapeHtml(node.description)}</p>
            </div>
        ` : ''}
        
        ${node.tags && node.tags.length > 0 ? `
            <div class="node-detail-section">
                <h3 class="node-detail-section-title">Tags</h3>
                <div class="node-detail-tags">
                    ${node.tags.map(tag => `<span class="tag-badge">${escapeHtml(tag)}</span>`).join('')}
                </div>
            </div>
        ` : ''}
        
        ${node.assignee ? `
            <div class="node-detail-section">
                <h3 class="node-detail-section-title">Assignee</h3>
                <p class="node-detail-assignee">${escapeHtml(node.assignee)}</p>
            </div>
        ` : ''}
        
        <div class="node-detail-section">
            <h3 class="node-detail-section-title">Timeline</h3>
            <div class="node-detail-meta-grid">
                <div class="node-detail-meta-item">
                    <span class="node-detail-meta-label">Created:</span>
                    <span class="node-detail-meta-value">${formatDate(node.created_at)}</span>
                </div>
                <div class="node-detail-meta-item">
                    <span class="node-detail-meta-label">Updated:</span>
                    <span class="node-detail-meta-value">${formatDate(node.updated_at)}</span>
                </div>
                ${node.closed_at ? `
                    <div class="node-detail-meta-item">
                        <span class="node-detail-meta-label">Closed:</span>
                        <span class="node-detail-meta-value">${formatDate(node.closed_at)}</span>
                    </div>
                ` : ''}
            </div>
        </div>
        
        ${node.closed_reason ? `
            <div class="node-detail-section">
                <h3 class="node-detail-section-title">Close Reason</h3>
                <p class="node-detail-close-reason">${escapeHtml(node.closed_reason)}</p>
            </div>
        ` : ''}
        
        ${node.edges && node.edges.length > 0 ? `
            <div class="node-detail-section">
                <h3 class="node-detail-section-title">Relationships</h3>
                <div class="node-detail-edges">
                    ${node.edges.map(edge => `
                        <div class="node-detail-edge">
                            <span class="edge-type">${escapeHtml(edge.edge_type)}</span>
                            <span class="edge-target">${escapeHtml(edge.related_id)}</span>
                            ${edge.related_title ? `<span class="edge-title">${escapeHtml(edge.related_title)}</span>` : ''}
                        </div>
                    `).join('')}
                </div>
            </div>
        ` : ''}
    `;
    
    return html;
}

/**
 * Render content for a test node
 * @param {Object} node - Test node
 * @returns {string} HTML content
 */
function renderTestContent(node) {
    return `
        ${node.description ? `
            <div class="node-detail-section">
                <h3 class="node-detail-section-title">Description</h3>
                <p class="node-detail-description">${escapeHtml(node.description)}</p>
            </div>
        ` : ''}
        
        <div class="node-detail-section">
            <h3 class="node-detail-section-title">Test Configuration</h3>
            <div class="node-detail-meta-grid">
                ${node.command ? `
                    <div class="node-detail-meta-item full-width">
                        <span class="node-detail-meta-label">Command:</span>
                        <code class="node-detail-code">${escapeHtml(node.command)}</code>
                    </div>
                ` : ''}
                ${node.working_dir ? `
                    <div class="node-detail-meta-item">
                        <span class="node-detail-meta-label">Working Directory:</span>
                        <code class="node-detail-code">${escapeHtml(node.working_dir)}</code>
                    </div>
                ` : ''}
                ${node.pattern ? `
                    <div class="node-detail-meta-item">
                        <span class="node-detail-meta-label">Pattern:</span>
                        <code class="node-detail-code">${escapeHtml(node.pattern)}</code>
                    </div>
                ` : ''}
            </div>
        </div>
        
        <div class="node-detail-section">
            <h3 class="node-detail-section-title">Timeline</h3>
            <div class="node-detail-meta-grid">
                <div class="node-detail-meta-item">
                    <span class="node-detail-meta-label">Created:</span>
                    <span class="node-detail-meta-value">${formatDate(node.created_at)}</span>
                </div>
            </div>
        </div>
        
        ${node.linked_tasks && node.linked_tasks.length > 0 ? `
            <div class="node-detail-section">
                <h3 class="node-detail-section-title">Linked Tasks</h3>
                <div class="node-detail-linked-tasks">
                    ${node.linked_tasks.map(taskId => `
                        <div class="linked-task-item">${escapeHtml(taskId)}</div>
                    `).join('')}
                </div>
            </div>
        ` : ''}
    `;
}

/**
 * Render content for an idea node
 * @param {Object} node - Idea node
 * @returns {string} HTML content
 */
function renderIdeaContent(node) {
    return `
        <div class="node-detail-section">
            <div class="node-detail-badges">
                ${getStatusBadgeHTML(node.status, 'idea')}
            </div>
        </div>
        
        ${node.description ? `
            <div class="node-detail-section">
                <h3 class="node-detail-section-title">Description</h3>
                <p class="node-detail-description">${escapeHtml(node.description)}</p>
            </div>
        ` : ''}
        
        ${node.tags && node.tags.length > 0 ? `
            <div class="node-detail-section">
                <h3 class="node-detail-section-title">Tags</h3>
                <div class="node-detail-tags">
                    ${node.tags.map(tag => `<span class="tag-badge">${escapeHtml(tag)}</span>`).join('')}
                </div>
            </div>
        ` : ''}
        
        <div class="node-detail-section">
            <h3 class="node-detail-section-title">Timeline</h3>
            <div class="node-detail-meta-grid">
                <div class="node-detail-meta-item">
                    <span class="node-detail-meta-label">Created:</span>
                    <span class="node-detail-meta-value">${formatDate(node.created_at)}</span>
                </div>
                <div class="node-detail-meta-item">
                    <span class="node-detail-meta-label">Updated:</span>
                    <span class="node-detail-meta-value">${formatDate(node.updated_at)}</span>
                </div>
            </div>
        </div>
    `;
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
 * Get node type display name and color
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
        queue: { name: 'Queue', color: 'var(--accent-blue)' }
    };
    
    return types[type] || { name: type, color: 'var(--text-secondary)' };
}

/**
 * Show the node detail modal with node information
 * @param {string} nodeId - The node ID
 */
export async function showNodeDetailModal(nodeId) {
    const overlay = document.getElementById('node-detail-modal');
    if (!overlay) {
        console.error('Node detail modal overlay not found in DOM');
        return;
    }
    
    const node = getNode(nodeId);
    if (!node) {
        console.error(`Node ${nodeId} not found`);
        return;
    }
    
    // Update title and badges
    const titleEl = document.getElementById('node-detail-modal-title');
    const typeEl = document.getElementById('node-detail-modal-type-badge');
    const idEl = document.getElementById('node-detail-modal-id');
    
    titleEl.textContent = node.title || node.name || 'Untitled';
    
    const typeInfo = getNodeTypeInfo(node.type);
    typeEl.textContent = typeInfo.name;
    typeEl.style.background = typeInfo.color;
    
    idEl.textContent = '';
    idEl.appendChild(createClickableId(node.id));
    
    // Update content based on node type
    const contentEl = document.getElementById('node-detail-modal-content');
    
    let contentHTML = '';
    if (node.type === 'doc') {
        contentHTML = renderDocContent(node);
    } else if (node.type === 'task' || node.type === 'bug' || node.type === 'milestone') {
        contentHTML = renderTaskContent(node);
    } else if (node.type === 'test') {
        contentHTML = renderTestContent(node);
    } else if (node.type === 'idea') {
        contentHTML = renderIdeaContent(node);
    } else {
        contentHTML = '<div class="node-detail-empty">No details available for this node type.</div>';
    }
    
    contentEl.innerHTML = contentHTML;
    
    // Make all binnacle IDs in the content clickable
    contentEl.querySelectorAll('.edge-target').forEach(el => {
        makeIdsClickable(el);
    });
    
    // Show overlay immediately with loading state for docs
    overlay.classList.remove('hidden');
    
    // If it's a doc, fetch full content
    if (node.type === 'doc') {
        const markdownEl = document.getElementById('node-detail-markdown-content');
        if (markdownEl) {
            try {
                // Show loading state
                markdownEl.innerHTML = '<div class="node-detail-loading">Loading document...</div>';
                
                const response = await fetch(`/api/docs/${nodeId}`);
                if (!response.ok) {
                    throw new Error(`Failed to fetch document: ${response.status}`);
                }
                const data = await response.json();
                const fullDoc = data.doc;
                
                if (fullDoc.content) {
                    renderMarkdown(markdownEl, fullDoc.content);
                } else {
                    markdownEl.innerHTML = '<p class="node-detail-empty">This document has no content yet.</p>';
                }
            } catch (error) {
                console.error('Error loading document:', error);
                markdownEl.innerHTML = '<p class="node-detail-empty">Error loading document. Please try again.</p>';
            }
        }
    }
}

/**
 * Hide the node detail modal
 */
export function hideNodeDetailModal() {
    const overlay = document.getElementById('node-detail-modal');
    if (overlay) {
        overlay.classList.add('hidden');
    }
}

/**
 * Initialize the node detail modal with event handlers
 */
export function initNodeDetailModal() {
    const overlay = document.getElementById('node-detail-modal');
    if (!overlay) {
        console.error('Node detail modal overlay not found in DOM');
        return;
    }
    
    // Close button
    const closeBtn = document.getElementById('node-detail-modal-close');
    closeBtn.addEventListener('click', hideNodeDetailModal);
    
    // Close on overlay click (but not on content click)
    overlay.addEventListener('click', (e) => {
        if (e.target === overlay) {
            hideNodeDetailModal();
        }
    });
    
    // Close on Escape key
    document.addEventListener('keydown', (e) => {
        if (e.key === 'Escape' && !overlay.classList.contains('hidden')) {
            hideNodeDetailModal();
        }
    });
}

/**
 * Mount the node detail modal to the DOM
 * @param {HTMLElement|string} target - Target element or selector
 */
export function mountNodeDetailModal(target) {
    const container = typeof target === 'string' 
        ? document.querySelector(target) 
        : target;
    
    if (!container) {
        console.error('Node detail modal target not found');
        return;
    }
    
    const modal = createNodeDetailModal();
    container.appendChild(modal);
    initNodeDetailModal();
}
