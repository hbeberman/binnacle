/**
 * Workflow Pane Component
 * 
 * Combined view of ready work and recently completed items.
 * Displays a unified "workflow view" showing:
 * - Available work (ready tasks and open bugs)
 * - Recently completed items (done tasks and bugs)
 * 
 * This gives operators a single view of the workflow cycle.
 */

import { 
    subscribe, 
    getReady,
    getBugs,
    getTasks
} from '../state.js';
import { createClickableId } from '../utils/clickable-ids.js';

/**
 * Default number of items to show per section
 */
const DEFAULT_READY_LIMIT = 5;
const DEFAULT_COMPLETED_LIMIT = 3;

/**
 * Format time since completion to human-readable string
 * @param {string} closedAt - ISO timestamp of when task was closed
 * @returns {string} Formatted time ago (e.g., "2h ago", "3d ago", "just now")
 */
function formatTimeAgo(closedAt) {
    if (!closedAt) return '';
    
    const closedTime = new Date(closedAt);
    const now = new Date();
    const ms = now - closedTime;
    
    if (ms < 0) return 'just now';
    
    const seconds = Math.floor(ms / 1000);
    const minutes = Math.floor(seconds / 60);
    const hours = Math.floor(minutes / 60);
    const days = Math.floor(hours / 24);
    
    if (days > 0) return `${days}d ago`;
    if (hours > 0) return `${hours}h ago`;
    if (minutes > 0) return `${minutes}m ago`;
    if (seconds > 10) return `${seconds}s ago`;
    return 'just now';
}

/**
 * Get emoji for priority level
 * @param {number} priority - Priority level (0-4)
 * @returns {string} Priority indicator
 */
function getPriorityIndicator(priority) {
    if (priority === 0) return '🔴'; // Critical
    if (priority === 1) return '🟠'; // High
    return ''; // Normal (P2+) - no indicator
}

/**
 * Get type indicator emoji
 * @param {string} type - Entity type (task, bug, idea)
 * @returns {string} Type emoji
 */
function getTypeEmoji(type) {
    if (type === 'bug') return '🐛';
    if (type === 'idea') return '💡';
    return '📋'; // task
}

/**
 * Get ready tasks (non-bugs, non-ideas)
 * @param {number} limit - Max items to return
 * @returns {Array} Ready tasks
 */
function getReadyTasks(limit = DEFAULT_READY_LIMIT) {
    const ready = getReady() || [];
    // Filter to just tasks (exclude bugs and ideas - they're shown separately)
    const tasks = ready.filter(t => t.type !== 'bug' && t.type !== 'idea');
    // Sort by priority (lower = higher priority), then by queued status
    tasks.sort((a, b) => {
        // Queued items first
        if (a.queued && !b.queued) return -1;
        if (!a.queued && b.queued) return 1;
        // Then by priority
        return (a.priority || 2) - (b.priority || 2);
    });
    return tasks.slice(0, limit);
}

/**
 * Get open bugs
 * @param {number} limit - Max items to return
 * @returns {Array} Open bugs
 */
function getOpenBugs(limit = 5) {
    const allBugs = getBugs() || [];
    const openBugs = allBugs.filter(b => 
        b.status !== 'done' && 
        b.status !== 'cancelled'
    );
    // Sort by severity/priority
    openBugs.sort((a, b) => {
        // Critical/high severity first
        const severityOrder = { critical: 0, high: 1, medium: 2, low: 3, triage: 4 };
        const aSev = severityOrder[a.severity] ?? 4;
        const bSev = severityOrder[b.severity] ?? 4;
        if (aSev !== bSev) return aSev - bSev;
        return (a.priority || 2) - (b.priority || 2);
    });
    return openBugs.slice(0, limit);
}

/**
 * Get recently completed items (tasks and bugs)
 * @param {number} limit - Maximum number of items to return
 * @returns {Array} Array of completed items, sorted by closed_at desc
 */
function getRecentlyCompleted(limit = DEFAULT_COMPLETED_LIMIT) {
    const tasks = getTasks() || [];
    const bugs = getBugs() || [];
    
    // Combine and filter for done items with closed_at
    const completed = [...tasks, ...bugs].filter(item => 
        item.status === 'done' && item.closed_at
    );
    
    // Sort by closed_at descending (most recent first)
    completed.sort((a, b) => {
        const aTime = new Date(a.closed_at);
        const bTime = new Date(b.closed_at);
        return bTime - aTime;
    });
    
    return completed.slice(0, limit);
}

/**
 * Create a work item row element
 * @param {Object} item - The task or bug
 * @param {string} section - Section type ('ready', 'bug', 'completed')
 * @returns {HTMLElement} Row element
 */
function createWorkItem(item, section) {
    const row = document.createElement('div');
    row.className = `workflow-item type-${item.type} section-${section}`;
    if (item.queued) {
        row.classList.add('queued');
    }
    
    // Left side: type emoji + ID
    const leftEl = document.createElement('div');
    leftEl.className = 'workflow-item-left';
    
    const typeEmoji = document.createElement('span');
    typeEmoji.className = 'workflow-item-type';
    typeEmoji.textContent = getTypeEmoji(item.type);
    
    const idEl = createClickableId(item.id);
    idEl.className = 'workflow-item-id clickable-id';
    
    leftEl.appendChild(typeEmoji);
    leftEl.appendChild(idEl);
    
    // Middle: title (with priority indicator for high-priority items)
    const titleEl = document.createElement('div');
    titleEl.className = 'workflow-item-title';
    const priorityIndicator = getPriorityIndicator(item.priority);
    const displayTitle = item.short_name || item.title;
    titleEl.innerHTML = priorityIndicator ? 
        `<span class="priority-indicator">${priorityIndicator}</span> ${escapeHtml(displayTitle)}` :
        escapeHtml(displayTitle);
    titleEl.title = item.title;
    
    // Right side: time ago (for completed) or queued badge (for ready)
    const rightEl = document.createElement('div');
    rightEl.className = 'workflow-item-right';
    
    if (section === 'completed' && item.closed_at) {
        rightEl.textContent = formatTimeAgo(item.closed_at);
        rightEl.title = `Completed ${new Date(item.closed_at).toLocaleString()}`;
    } else if (item.queued) {
        const badge = document.createElement('span');
        badge.className = 'queued-badge';
        badge.textContent = 'QUEUED';
        badge.title = 'Prioritized in work queue';
        rightEl.appendChild(badge);
    }
    
    row.appendChild(leftEl);
    row.appendChild(titleEl);
    row.appendChild(rightEl);
    
    return row;
}

/**
 * Simple HTML escaping
 * @param {string} str - String to escape
 * @returns {string} Escaped string
 */
function escapeHtml(str) {
    const div = document.createElement('div');
    div.textContent = str;
    return div.innerHTML;
}

/**
 * Update the pane display
 * @param {HTMLElement} pane - The pane element
 * @param {Object} options - Configuration options
 */
function updatePane(pane, options) {
    const readyLimit = options.readyLimit || DEFAULT_READY_LIMIT;
    const completedLimit = options.completedLimit || DEFAULT_COMPLETED_LIMIT;
    
    const readyTasks = getReadyTasks(readyLimit);
    const openBugs = getOpenBugs(5);
    const completedItems = getRecentlyCompleted(completedLimit);
    
    const totalReady = readyTasks.length + openBugs.length;
    const hasReady = totalReady > 0;
    const hasCompleted = completedItems.length > 0;
    
    // Update pane classes
    pane.classList.toggle('has-ready', hasReady);
    pane.classList.toggle('has-completed', hasCompleted);
    pane.classList.toggle('empty', !hasReady && !hasCompleted);
    
    // Build content
    let html = `
        <div class="workflow-pane-header">
            <span class="workflow-pane-title">Workflow</span>
            <span class="workflow-pane-counts">
                ${hasReady ? `<span class="ready-count">${totalReady} ready</span>` : ''}
                ${hasReady && hasCompleted ? '<span class="count-separator">·</span>' : ''}
                ${hasCompleted ? `<span class="completed-count">${completedItems.length} done</span>` : ''}
            </span>
        </div>
    `;
    
    // Ready section
    if (hasReady) {
        html += `<div class="workflow-section ready-section">`;
        html += `<div class="workflow-section-header">Ready to Work</div>`;
        html += `<div class="workflow-section-items">`;
        
        // Show tasks first
        for (const task of readyTasks) {
            html += createWorkItem(task, 'ready').outerHTML;
        }
        
        // Then bugs
        for (const bug of openBugs) {
            html += createWorkItem(bug, 'bug').outerHTML;
        }
        
        html += `</div></div>`;
    }
    
    // Completed section
    if (hasCompleted) {
        html += `<div class="workflow-section completed-section">`;
        html += `<div class="workflow-section-header">Recently Completed</div>`;
        html += `<div class="workflow-section-items">`;
        
        for (const item of completedItems) {
            html += createWorkItem(item, 'completed').outerHTML;
        }
        
        html += `</div></div>`;
    }
    
    // Empty state
    if (!hasReady && !hasCompleted) {
        html += `<div class="workflow-empty">No tasks ready or recently completed</div>`;
    }
    
    pane.innerHTML = html;
    
    // Note: Clickable IDs use event delegation set up in createWorkflowPane,
    // so no need to re-attach handlers after innerHTML update.
}

/**
 * Create the workflow pane element
 * @param {Object} options - Configuration options
 * @param {number} options.readyLimit - Max ready items to show (default: 5)
 * @param {number} options.completedLimit - Max completed items to show (default: 3)
 * @returns {HTMLElement} The pane element
 */
export function createWorkflowPane(options = {}) {
    const pane = document.createElement('div');
    pane.className = 'workflow-pane empty';
    pane.id = 'workflow-pane';
    
    // Add event delegation for clickable IDs
    pane.addEventListener('click', (e) => {
        const clickableId = e.target.closest('.clickable-id');
        if (clickableId) {
            const entityId = clickableId.dataset.entityId || clickableId.textContent.trim();
            if (entityId) {
                // Dispatch custom event for node selection
                const event = new CustomEvent('entity-id-click', {
                    bubbles: true,
                    detail: { entityId }
                });
                pane.dispatchEvent(event);
            }
        }
    });
    
    // Update function
    const update = () => updatePane(pane, options);
    
    // Subscribe to state changes
    subscribe('ready', update);
    subscribe('entities.tasks', update);
    subscribe('entities.bugs', update);
    
    // Initialize with current state
    update();
    
    return pane;
}

/**
 * Mount the workflow pane to a container
 * @param {HTMLElement|string} target - Target container element or selector
 * @param {Object} options - Configuration options
 * @returns {HTMLElement|null} The pane element, or null if target not found
 */
export function mountWorkflowPane(target, options = {}) {
    const container = typeof target === 'string'
        ? document.querySelector(target)
        : target;
    
    if (!container) {
        console.warn('Workflow pane: target container not found');
        return null;
    }
    
    const pane = createWorkflowPane(options);
    container.appendChild(pane);
    
    return pane;
}
