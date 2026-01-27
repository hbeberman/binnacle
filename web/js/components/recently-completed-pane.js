/**
 * Recently Completed Pane Component
 * 
 * Displays the last N completed tasks/bugs with:
 * - Task/bug ID (clickable)
 * - Short name or title
 * - Time since completion
 * 
 * Updates automatically when entities change.
 */

import { 
    subscribe, 
    getTasks,
    getBugs
} from '../state.js';
import { createClickableId } from '../utils/clickable-ids.js';

/**
 * Default number of items to show
 */
const DEFAULT_ITEMS_TO_SHOW = 5;

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
 * Get recently completed items (tasks and bugs)
 * @param {number} limit - Maximum number of items to return
 * @returns {Array} Array of completed items, sorted by closed_at desc
 */
function getRecentlyCompleted(limit = DEFAULT_ITEMS_TO_SHOW) {
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
 * Create a completed item row element
 * @param {Object} item - The completed task or bug
 * @returns {HTMLElement} Row element
 */
function createCompletedItem(item) {
    const row = document.createElement('div');
    row.className = `completed-item type-${item.type}`;
    
    const idEl = createClickableId(item.id);
    idEl.className = 'completed-item-id clickable-id';
    
    const titleEl = document.createElement('span');
    titleEl.className = 'completed-item-title';
    titleEl.textContent = item.short_name || item.title;
    titleEl.title = item.title;
    
    const timeEl = document.createElement('span');
    timeEl.className = 'completed-item-time';
    timeEl.textContent = formatTimeAgo(item.closed_at);
    timeEl.title = `Completed ${new Date(item.closed_at).toLocaleString()}`;
    
    row.appendChild(idEl);
    row.appendChild(titleEl);
    row.appendChild(timeEl);
    
    return row;
}

/**
 * Update the pane display
 * @param {HTMLElement} pane - The pane element
 * @param {number} limit - Maximum number of items to show
 */
function updatePane(pane, limit) {
    const listEl = pane.querySelector('.completed-list');
    const completed = getRecentlyCompleted(limit);
    
    if (completed.length === 0) {
        // No completed items - show placeholder
        pane.classList.add('no-items');
        pane.classList.remove('has-items');
        listEl.innerHTML = '<div class="completed-placeholder">No recently completed items</div>';
        return;
    }
    
    // Has completed items - show list
    pane.classList.remove('no-items');
    pane.classList.add('has-items');
    
    listEl.innerHTML = '';
    for (const item of completed) {
        listEl.appendChild(createCompletedItem(item));
    }
}

/**
 * Create the recently completed pane element
 * @param {Object} options - Configuration options
 * @param {number} options.limit - Max number of items to show (default: 5)
 * @returns {HTMLElement} The pane element
 */
export function createRecentlyCompletedPane(options = {}) {
    const limit = options.limit || DEFAULT_ITEMS_TO_SHOW;
    
    const pane = document.createElement('div');
    pane.className = 'recently-completed-pane no-items';
    pane.id = 'recently-completed-pane';
    
    pane.innerHTML = `
        <div class="recently-completed-pane-header">
            <span>Recently Completed</span>
        </div>
        <div class="completed-list"></div>
    `;
    
    // Update function
    const update = () => updatePane(pane, limit);
    
    // Subscribe to state changes that affect completed items
    subscribe('entities.tasks', update);
    subscribe('entities.bugs', update);
    
    // Initialize with current state
    update();
    
    return pane;
}

/**
 * Mount the recently completed pane to a container
 * @param {HTMLElement|string} target - Target container element or selector
 * @param {Object} options - Configuration options
 * @returns {HTMLElement|null} The pane element, or null if target not found
 */
export function mountRecentlyCompletedPane(target, options = {}) {
    const container = typeof target === 'string'
        ? document.querySelector(target)
        : target;
    
    if (!container) {
        console.warn('Recently completed pane: target container not found');
        return null;
    }
    
    const pane = createRecentlyCompletedPane(options);
    container.appendChild(pane);
    
    return pane;
}
