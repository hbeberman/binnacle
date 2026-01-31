/**
 * Node List Component
 * 
 * Renders all nodes in a list view with filtering and search.
 */

import * as state from '../state.js';
import { makeIdsClickable } from '../utils/clickable-ids.js';

/**
 * Check if a PRD doc node has any uncompleted milestones linked to it.
 * Returns true if the PRD should be visible (has active milestones).
 * @param {Object} prdNode - The PRD doc node
 * @returns {boolean} True if PRD has uncompleted milestones
 */
function prdHasActiveWork(prdNode) {
    const edges = state.get('edges') || [];
    const milestones = state.getMilestones() || [];
    
    // Build a set of milestone IDs that are children of this PRD
    const childMilestoneIds = new Set();
    for (const edge of edges) {
        // child_of edges: source is the child, target is the parent
        // So milestone -> child_of -> PRD means edge.source=milestone, edge.target=PRD
        if (edge.edge_type === 'child_of' && edge.target === prdNode.id) {
            childMilestoneIds.add(edge.source);
        }
    }
    
    // Check if any child milestones are uncompleted
    for (const milestone of milestones) {
        if (childMilestoneIds.has(milestone.id)) {
            if (milestone.status !== 'done' && milestone.status !== 'cancelled') {
                return true;
            }
        }
    }
    
    return false;
}

/**
 * Initialize the node list view
 * @param {string|HTMLElement} containerSelector - Container element or selector
 * @param {Object} options - Configuration options
 * @param {Function} options.onNodeClick - Callback when a node is clicked
 * @param {Function} options.onDocRead - Callback when a doc's Read button is clicked (docId)
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
    state.subscribe('ui.searchQuery', () => renderNodeList(container, options));
    state.subscribe('ui.hideCompleted', () => renderNodeList(container, options));
    
    // Initial render
    renderNodeList(container, options);
}

/**
 * Render the node list as a kanban board
 */
function renderNodeList(container, options = {}) {
    const searchQuery = state.get('ui.searchQuery');
    const hideCompleted = state.get('ui.hideCompleted');
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
    
    // Issues
    (state.getIssues() || []).forEach(i => {
        allNodes.push({ ...i, nodeType: 'issue' });
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
    
    // Apply hideCompleted filter
    if (hideCompleted) {
        allNodes = allNodes.filter(node => {
            // For PRD docs: only hide if they have no uncompleted milestones
            // For other docs: hide by default
            if (node.nodeType === 'doc') {
                if (node.doc_type === 'prd') {
                    return prdHasActiveWork(node);
                }
                return false;
            }
            
            // Keep tests (no status concept)
            if (node.nodeType === 'test') {
                return true;
            }
            
            // Filter out completed statuses for ideas
            if (node.nodeType === 'idea') {
                return node.status !== 'promoted' && node.status !== 'wilted';
            }
            
            // Filter out completed statuses for issues
            if (node.nodeType === 'issue') {
                return !['resolved', 'closed', 'wont_fix', 'by_design', 'no_repro'].includes(node.status);
            }
            
            // Filter out completed statuses for tasks/bugs/milestones
            return node.status !== 'done' && node.status !== 'cancelled';
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
    
    // Group nodes by status (kanban columns)
    // Note: in_progress items are kept in the pending column with a banner
    // to show all actionable work together
    const columns = {
        pending: [],
        blocked: [],
        done: []
    };
    
    allNodes.forEach(node => {
        if (node.nodeType === 'task' || node.nodeType === 'bug' || node.nodeType === 'milestone') {
            if (node.status === 'done' || node.status === 'cancelled') {
                columns.done.push(node);
            } else if (node.status === 'blocked') {
                columns.blocked.push(node);
            } else {
                // pending and in_progress go together in pending column
                // in_progress items will have a banner to distinguish them
                columns.pending.push(node);
            }
        } else if (node.nodeType === 'issue') {
            // Issues have their own status workflow
            if (['resolved', 'closed', 'wont_fix', 'by_design', 'no_repro'].includes(node.status)) {
                columns.done.push(node);
            } else {
                // open, triage, investigating all go in pending
                // investigating will have an in-progress banner
                columns.pending.push(node);
            }
        } else if (node.nodeType === 'idea') {
            if (node.status === 'promoted' || node.status === 'wilted') {
                columns.done.push(node);
            } else {
                columns.pending.push(node);
            }
        } else {
            // Tests and docs go in pending by default
            columns.pending.push(node);
        }
    });
    
    // Sort each column
    Object.values(columns).forEach(column => {
        column.sort((a, b) => {
            // In-progress items first (they have banner and should be prominent)
            const aInProgress = (a.status === 'in_progress' || a.status === 'investigating') ? 0 : 1;
            const bInProgress = (b.status === 'in_progress' || b.status === 'investigating') ? 0 : 1;
            if (aInProgress !== bInProgress) return aInProgress - bInProgress;
            
            // Ready items next
            const aReady = readyIds.has(a.id);
            const bReady = readyIds.has(b.id);
            if (aReady !== bReady) return bReady - aReady;
            
            // Priority
            if ((a.priority ?? 2) !== (b.priority ?? 2)) {
                return (a.priority ?? 2) - (b.priority ?? 2);
            }
            
            return a.id.localeCompare(b.id);
        });
    });
    
    // Render kanban board
    if (allNodes.length === 0) {
        container.innerHTML = searchQuery 
            ? '<div class="empty-state">No nodes match your search</div>'
            : '<div class="empty-state">No open nodes</div>';
        return;
    }
    
    // Build array of non-empty columns to render
    const columnsToRender = [];
    if (columns.pending.length > 0) {
        columnsToRender.push(renderKanbanColumn('Pending', 'pending', columns.pending, readyIds, options));
    }
    if (columns.blocked.length > 0) {
        columnsToRender.push(renderKanbanColumn('Blocked', 'blocked', columns.blocked, readyIds, options));
    }
    if (columns.done.length > 0) {
        columnsToRender.push(renderKanbanColumn('Done', 'done', columns.done, readyIds, options));
    }
    
    container.innerHTML = `
        <div class="kanban-board">
            ${columnsToRender.join('')}
        </div>
    `;
    
    // Make all badge-id elements clickable
    container.querySelectorAll('.badge-id').forEach(el => {
        makeIdsClickable(el);
    });
    
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
    
    // Attach doc read button handlers
    container.querySelectorAll('.card-read-btn').forEach(btn => {
        btn.addEventListener('click', (e) => {
            e.stopPropagation(); // Prevent triggering card click
            const docId = e.currentTarget.getAttribute('data-doc-id');
            if (docId && options.onDocRead) {
                options.onDocRead(docId);
            }
        });
    });
    
    // Add double-click handler to cards
    container.querySelectorAll('.node-card').forEach(card => {
        card.addEventListener('dblclick', (_e) => {
            const nodeId = card.getAttribute('data-node-id');
            if (nodeId && options.onInfoClick) {
                options.onInfoClick(nodeId);
            }
        });
    });
}

/**
 * Render a single kanban column
 */
function renderKanbanColumn(title, columnId, nodes, readyIds, options) {
    return `
        <div class="kanban-column" data-column="${columnId}">
            <div class="kanban-column-header">
                <h3 class="kanban-column-title">${title}</h3>
                <span class="kanban-column-count">${nodes.length}</span>
            </div>
            <div class="kanban-column-content">
                ${nodes.length === 0 
                    ? '<div class="kanban-column-empty">No items</div>'
                    : nodes.map(node => renderNodeCard(node, readyIds, options)).join('')
                }
            </div>
        </div>
    `;
}

/**
 * Render a single node card
 */
function renderNodeCard(node, readyIds, _options) {
    switch (node.nodeType) {
        case 'task':
        case 'bug':
            return renderTaskBugCard(node, readyIds);
        case 'issue':
            return renderIssueCard(node);
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
    <div class="node-card ${isBlocked ? 'card-blocked' : ''} ${isClosed ? 'card-closed' : ''} ${isInProgress ? 'card-in-progress' : ''}" data-node-id="${node.id}">
        ${isClosed ? `<div class="closed-banner">✓ ${node.status === 'done' ? (isBug ? 'Fixed' : 'Done') : 'Cancelled'}</div>` : ''}
        ${isInProgress ? `<div class="in-progress-banner">⚡ In Progress</div>` : ''}
        ${isBlocked ? `<div class="blocked-banner">🚫 Blocked</div>` : ''}
        <div class="card-header">
            <div class="card-title">${isBug ? '🐛 ' : '📋 '}${escapeHtml(node.title)}</div>
            <div class="card-actions">
                <button class="card-jump-btn" data-node-id="${node.id}" title="Jump to graph">🎯</button>
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
 * Render issue card
 */
function renderIssueCard(issue) {
    const isClosed = ['resolved', 'closed', 'wont_fix', 'by_design', 'no_repro'].includes(issue.status);
    const isInvestigating = issue.status === 'investigating';
    const statusLabel = issue.status === 'open' ? '❓ Open'
        : issue.status === 'triage' ? '🔍 Triage'
        : issue.status === 'investigating' ? '🔬 Investigating'
        : issue.status === 'resolved' ? '✅ Resolved'
        : issue.status === 'closed' ? '✓ Closed'
        : issue.status === 'wont_fix' ? '⛔ Won\'t Fix'
        : issue.status === 'by_design' ? '🎨 By Design'
        : issue.status === 'no_repro' ? '🔎 No Repro'
        : issue.status;
    
    return `
    <div class="node-card ${isClosed ? 'card-closed' : ''} ${isInvestigating ? 'card-in-progress' : ''}" data-node-id="${issue.id}">
        ${isClosed ? `<div class="closed-banner">${statusLabel}</div>` : ''}
        ${isInvestigating ? `<div class="in-progress-banner">🔬 Investigating</div>` : ''}
        <div class="card-header">
            <div class="card-title">❓ ${escapeHtml(issue.title)}</div>
            <div class="card-actions">
                <button class="card-jump-btn" data-node-id="${issue.id}" title="Jump to graph">🎯</button>
            </div>
        </div>
        ${issue.description ? `<div class="card-description">${escapeHtml(issue.description)}</div>` : ''}
        <div class="card-meta">
            <span class="badge badge-priority-${issue.priority ?? 2}">P${issue.priority ?? 2}</span>
            <span class="badge">${statusLabel}</span>
            <span class="badge badge-id">${issue.id}</span>
            ${(issue.tags || []).map(tag => `<span class="badge badge-tag">${escapeHtml(tag)}</span>`).join('')}
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
    <div class="node-card ${isClosed ? 'card-closed' : ''}" data-node-id="${idea.id}">
        ${isClosed ? `<div class="closed-banner">✓ ${idea.status === 'promoted' ? 'Promoted' : 'Wilted'}</div>` : ''}
        <div class="card-header">
            <div class="card-title">💡 ${escapeHtml(idea.title)}</div>
            <div class="card-actions">
                <button class="card-jump-btn" data-node-id="${idea.id}" title="Jump to graph">🎯</button>
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
    <div class="node-card" data-node-id="${test.id}">
        <div class="card-header">
            <div class="card-title">🧪 ${escapeHtml(test.name)}</div>
            <div class="card-actions">
                <button class="card-jump-btn" data-node-id="${test.id}" title="Jump to graph">🎯</button>
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
    <div class="node-card" data-node-id="${doc.id}">
        <div class="card-header">
            <div class="card-title">${docTypeLabel} ${escapeHtml(doc.title)}</div>
            <div class="card-actions">
                <button class="card-read-btn" data-doc-id="${doc.id}" title="Open document viewer">View</button>
                <button class="card-jump-btn" data-node-id="${doc.id}" title="Jump to graph">🎯</button>
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
    <div class="node-card ${isClosed ? 'card-closed' : ''}" data-node-id="${milestone.id}">
        ${isClosed ? `<div class="closed-banner">✓ ${milestone.status === 'done' ? 'Done' : 'Cancelled'}</div>` : ''}
        <div class="card-header">
            <div class="card-title">🎯 ${escapeHtml(milestone.title)}</div>
            <div class="card-actions">
                <button class="card-jump-btn" data-node-id="${milestone.id}" title="Jump to graph">🎯</button>
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
