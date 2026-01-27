/**
 * Node List Component
 * 
 * Renders all nodes in a list view with filtering and search.
 */

import * as state from '../state.js';

/**
 * Initialize the node list view
 * @param {string|HTMLElement} containerSelector - Container element or selector
 * @param {Object} options - Configuration options
 * @param {Function} options.onNodeClick - Callback when a node is clicked
 */
export function initializeNodeList(containerSelector, options = {}) {
    const container = typeof containerSelector === 'string'
        ? document.querySelector(containerSelector)
        : containerSelector;
    
    if (!container) {
        console.error('Node list container not found:', containerSelector);
        return;
    }
    
    // Subscribe to state changes and re-render
    state.subscribe('entities.*', () => renderNodeList(container, options));
    state.subscribe('ready', () => renderNodeList(container, options));
    state.subscribe('ui.hideCompleted', () => renderNodeList(container, options));
    state.subscribe('ui.searchQuery', () => renderNodeList(container, options));
    
    // Initial render
    renderNodeList(container, options);
}

/**
 * Render the node list
 */
function renderNodeList(container, options = {}) {
    const hideCompleted = state.get('ui.hideCompleted');
    const searchQuery = state.get('ui.searchQuery');
    const readyIds = new Set((state.getReady() || []).map(t => t.id));
    
    // Collect all nodes from different sources
    let allNodes = [];
    
    // Tasks
    (state.getTasks() || []).forEach(t => {
        allNodes.push({ ...t, nodeType: 'task' });
    });
    
    // Bugs
    (state.getBugs() || []).forEach(b => {
        allNodes.push({ ...b, nodeType: 'bug' });
    });
    
    // Ideas
    (state.getIdeas() || []).forEach(i => {
        allNodes.push({ ...i, nodeType: 'idea' });
    });
    
    // Tests
    (state.getTests() || []).forEach(t => {
        allNodes.push({ ...t, nodeType: 'test' });
    });
    
    // Docs
    (state.getDocs() || []).forEach(d => {
        allNodes.push({ ...d, nodeType: 'doc' });
    });
    
    // Milestones
    (state.getMilestones() || []).forEach(m => {
        allNodes.push({ ...m, nodeType: 'milestone' });
    });
    
    // Filter by completed status
    if (hideCompleted) {
        allNodes = allNodes.filter(node => {
            if (node.nodeType === 'task' || node.nodeType === 'bug' || node.nodeType === 'milestone') {
                return node.status !== 'done' && node.status !== 'cancelled';
            } else if (node.nodeType === 'idea') {
                return node.status === 'seed' || node.status === 'germinating';
            }
            // Tests and docs don't have a closed state
            return true;
        });
    }
    
    // Apply search filter
    if (searchQuery) {
        const query = searchQuery.toLowerCase();
        allNodes = allNodes.filter(node => {
            const title = (node.title || node.name || '').toLowerCase();
            const description = (node.description || '').toLowerCase();
            const id = (node.id || '').toLowerCase();
            const tags = (node.tags || []).join(' ').toLowerCase();
            return title.includes(query) || description.includes(query) || 
                   id.includes(query) || tags.includes(query);
        });
    }
    
    // Sort: open first, ready first, by priority, then by ID
    allNodes.sort((a, b) => {
        // Type ordering
        const typeOrder = { task: 0, bug: 1, milestone: 2, idea: 3, test: 4, doc: 5 };
        if (typeOrder[a.nodeType] !== typeOrder[b.nodeType]) {
            return (typeOrder[a.nodeType] ?? 99) - (typeOrder[b.nodeType] ?? 99);
        }
        
        // For tasks/bugs/milestones: open first, ready first, priority
        if (a.nodeType === 'task' || a.nodeType === 'bug' || a.nodeType === 'milestone') {
            const aOpen = a.status !== 'done' && a.status !== 'cancelled';
            const bOpen = b.status !== 'done' && b.status !== 'cancelled';
            if (aOpen !== bOpen) return bOpen - aOpen;
            
            const aReady = readyIds.has(a.id);
            const bReady = readyIds.has(b.id);
            if (aReady !== bReady) return bReady - aReady;
            
            if ((a.priority ?? 2) !== (b.priority ?? 2)) {
                return (a.priority ?? 2) - (b.priority ?? 2);
            }
        }
        
        // For ideas: open first
        if (a.nodeType === 'idea') {
            const aOpen = a.status === 'seed' || a.status === 'germinating';
            const bOpen = b.status === 'seed' || b.status === 'germinating';
            if (aOpen !== bOpen) return bOpen - aOpen;
        }
        
        return a.id.localeCompare(b.id);
    });
    
    // Render
    if (allNodes.length === 0) {
        container.innerHTML = searchQuery 
            ? '<div class="empty-state">No nodes match your search</div>'
            : '<div class="empty-state">No open nodes</div>';
        return;
    }
    
    container.innerHTML = allNodes.map(node => renderNodeCard(node, readyIds, options)).join('');
    
    // Attach event handlers
    container.querySelectorAll('.card-jump-btn').forEach(btn => {
        btn.addEventListener('click', (e) => {
            const nodeId = e.currentTarget.getAttribute('data-node-id');
            if (options.onNodeClick) {
                const node = state.getNode(nodeId);
                if (node) {
                    options.onNodeClick(node);
                }
            }
        });
    });
    
    container.querySelectorAll('.card-info-btn').forEach(btn => {
        btn.addEventListener('click', (e) => {
            const nodeId = e.currentTarget.getAttribute('data-node-id');
            if (options.onInfoClick) {
                options.onInfoClick(nodeId);
            }
        });
    });
}

/**
 * Render a single node card
 */
function renderNodeCard(node, readyIds, options) {
    switch (node.nodeType) {
        case 'task':
        case 'bug':
            return renderTaskBugCard(node, readyIds);
        case 'idea':
            return renderIdeaCard(node);
        case 'test':
            return renderTestCard(node);
        case 'doc':
            return renderDocCard(node);
        case 'milestone':
            return renderMilestoneCard(node);
        default:
            return '';
    }
}

/**
 * Render task/bug card
 */
function renderTaskBugCard(node, readyIds) {
    const isReady = readyIds.has(node.id);
    const isClosed = node.status === 'done' || node.status === 'cancelled';
    const isInProgress = node.status === 'in_progress';
    const isBlocked = !isClosed && !isInProgress && (node.status === 'blocked' || !isReady);
    const isBug = node.nodeType === 'bug';
    
    return `
    <div class="node-card ${isBlocked ? 'card-blocked' : ''} ${isClosed ? 'card-closed' : ''} ${isInProgress ? 'card-in-progress' : ''}">
        ${isClosed ? `<div class="closed-banner">✓ ${node.status === 'done' ? (isBug ? 'Fixed' : 'Done') : 'Cancelled'}</div>` : ''}
        ${isInProgress ? `<div class="in-progress-banner">⚡ In Progress</div>` : ''}
        ${isBlocked ? `<div class="blocked-banner">🚫 Blocked</div>` : ''}
        <div class="card-header">
            <div class="card-title">${isBug ? '🐛 ' : '📋 '}${escapeHtml(node.title)}</div>
            <div class="card-actions">
                <button class="card-info-btn" data-node-id="${node.id}" title="View details">ℹ️</button>
                <button class="card-jump-btn" data-node-id="${node.id}" title="Jump to graph">📍</button>
            </div>
        </div>
        ${node.description ? `<div class="card-description">${escapeHtml(node.description)}</div>` : ''}
        <div class="card-meta">
            <span class="badge badge-priority-${node.priority ?? 2}">P${node.priority ?? 2}</span>
            <span class="badge badge-id">${node.id}</span>
            ${(node.tags || []).map(tag => `<span class="badge badge-tag">${escapeHtml(tag)}</span>`).join('')}
        </div>
    </div>`;
}

/**
 * Render idea card
 */
function renderIdeaCard(idea) {
    const isClosed = idea.status !== 'seed' && idea.status !== 'germinating';
    const statusLabel = idea.status === 'seed' ? '🌱 Seed' 
        : idea.status === 'germinating' ? '🌿 Germinating'
        : idea.status === 'promoted' ? '🚀 Promoted'
        : idea.status === 'wilted' ? '🥀 Wilted'
        : idea.status;
    
    return `
    <div class="node-card ${isClosed ? 'card-closed' : ''}">
        ${isClosed ? `<div class="closed-banner">✓ ${idea.status === 'promoted' ? 'Promoted' : 'Wilted'}</div>` : ''}
        <div class="card-header">
            <div class="card-title">💡 ${escapeHtml(idea.title)}</div>
            <div class="card-actions">
                <button class="card-info-btn" data-node-id="${idea.id}" title="View details">ℹ️</button>
                <button class="card-jump-btn" data-node-id="${idea.id}" title="Jump to graph">📍</button>
            </div>
        </div>
        ${idea.description ? `<div class="card-description">${escapeHtml(idea.description)}</div>` : ''}
        <div class="card-meta">
            <span class="badge">${statusLabel}</span>
            <span class="badge badge-id">${idea.id}</span>
            ${(idea.tags || []).map(tag => `<span class="badge badge-tag">${escapeHtml(tag)}</span>`).join('')}
        </div>
    </div>`;
}

/**
 * Render test card
 */
function renderTestCard(test) {
    return `
    <div class="node-card">
        <div class="card-header">
            <div class="card-title">🧪 ${escapeHtml(test.name)}</div>
            <div class="card-actions">
                <button class="card-info-btn" data-node-id="${test.id}" title="View details">ℹ️</button>
                <button class="card-jump-btn" data-node-id="${test.id}" title="Jump to graph">📍</button>
            </div>
        </div>
        <div class="card-meta">
            <span class="badge badge-id">${test.id}</span>
            ${test.last_status ? `<span class="badge ${test.last_status === 'passed' ? 'badge-success' : 'badge-error'}">${test.last_status}</span>` : ''}
        </div>
    </div>`;
}

/**
 * Render doc card
 */
function renderDocCard(doc) {
    const docTypeLabel = doc.doc_type === 'prd' ? '📄 PRD'
        : doc.doc_type === 'handoff' ? '🤝 Handoff'
        : '📝 Note';
    
    return `
    <div class="node-card">
        <div class="card-header">
            <div class="card-title">${docTypeLabel} ${escapeHtml(doc.title)}</div>
            <div class="card-actions">
                <button class="card-info-btn" data-node-id="${doc.id}" title="View details">ℹ️</button>
                <button class="card-jump-btn" data-node-id="${doc.id}" title="Jump to graph">📍</button>
            </div>
        </div>
        <div class="card-meta">
            <span class="badge badge-id">${doc.id}</span>
        </div>
    </div>`;
}

/**
 * Render milestone card
 */
function renderMilestoneCard(milestone) {
    const isClosed = milestone.status === 'done' || milestone.status === 'cancelled';
    
    return `
    <div class="node-card ${isClosed ? 'card-closed' : ''}">
        ${isClosed ? `<div class="closed-banner">✓ ${milestone.status === 'done' ? 'Done' : 'Cancelled'}</div>` : ''}
        <div class="card-header">
            <div class="card-title">🎯 ${escapeHtml(milestone.title)}</div>
            <div class="card-actions">
                <button class="card-info-btn" data-node-id="${milestone.id}" title="View details">ℹ️</button>
                <button class="card-jump-btn" data-node-id="${milestone.id}" title="Jump to graph">📍</button>
            </div>
        </div>
        ${milestone.description ? `<div class="card-description">${escapeHtml(milestone.description)}</div>` : ''}
        <div class="card-meta">
            <span class="badge badge-id">${milestone.id}</span>
        </div>
    </div>`;
}

/**
 * Escape HTML to prevent XSS
 */
function escapeHtml(text) {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
}
