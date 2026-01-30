/**
 * Active Task Pane Component
 * 
 * Displays the currently in_progress task with:
 * - Task ID (clickable)
 * - Task title
 * - Duration timer (live updates in websocket mode, static in archive mode)
 * 
 * Updates automatically when task status changes.
 */

import { 
    subscribe, 
    getTasks,
    getBugs,
    getMode,
    getEdges,
    getAgents,
    getNode,
    ConnectionMode
} from '../state.js';

/**
 * Track recently-removed working_on edge targets to filter from fallback path.
 * This prevents tasks from appearing as "active" briefly after their working_on
 * edge is removed but before the task status update propagates.
 * @type {Map<string, number>}
 */
const recentlyRemovedTargets = new Map();

/**
 * Grace period in milliseconds after which removed targets are forgotten.
 * Tasks that were unlinked more than this many ms ago will be shown
 * in the fallback path again (if still in_progress).
 */
export const REMOVAL_GRACE_PERIOD_MS = 5000;

/**
 * Mark a task as recently unlinked from a working_on edge.
 * Called by message-handlers when edge_removed fires for working_on edges.
 * @param {string} taskId - The task ID that was unlinked
 */
export function markRecentlyUnlinked(taskId) {
    recentlyRemovedTargets.set(taskId, Date.now());
    
    // Schedule cleanup after grace period
    setTimeout(() => {
        const removedAt = recentlyRemovedTargets.get(taskId);
        // Only delete if this is still the same removal (not re-added and re-removed)
        if (removedAt && (Date.now() - removedAt) >= REMOVAL_GRACE_PERIOD_MS) {
            recentlyRemovedTargets.delete(taskId);
        }
    }, REMOVAL_GRACE_PERIOD_MS + 100); // Small buffer to ensure cleanup
}

/**
 * Check if a task was recently unlinked and should be filtered from fallback.
 * @param {string} taskId - The task ID to check
 * @returns {boolean} True if the task was recently unlinked
 */
function isRecentlyUnlinked(taskId) {
    const removedAt = recentlyRemovedTargets.get(taskId);
    if (!removedAt) return false;
    return (Date.now() - removedAt) < REMOVAL_GRACE_PERIOD_MS;
}

/**
 * Format duration from milliseconds to human-readable string
 * @param {number} ms - Duration in milliseconds
 * @returns {string} Formatted duration (e.g., "2h 15m", "45m", "30s")
 */
function formatDuration(ms) {
    const seconds = Math.floor(ms / 1000);
    const minutes = Math.floor(seconds / 60);
    const hours = Math.floor(minutes / 60);
    const days = Math.floor(hours / 24);
    
    if (days > 0) {
        const remainingHours = hours % 24;
        return remainingHours > 0 ? `${days}d ${remainingHours}h` : `${days}d`;
    }
    if (hours > 0) {
        const remainingMinutes = minutes % 60;
        return remainingMinutes > 0 ? `${hours}h ${remainingMinutes}m` : `${hours}h`;
    }
    if (minutes > 0) {
        return `${minutes}m`;
    }
    return `${seconds}s`;
}

/**
 * Calculate elapsed time since task was started
 * @param {string} updatedAt - ISO timestamp of when task was marked in_progress
 * @returns {number} Milliseconds elapsed
 */
function calculateElapsed(updatedAt) {
    if (!updatedAt) return 0;
    const startTime = new Date(updatedAt);
    const now = new Date();
    return now - startTime;
}

/**
 * Find active tasks being worked on by agents
 * Uses working_on edges to determine which tasks are actively being worked on.
 * Returns an array of {agent, task, startTime} tuples.
 * @returns {Array} Array of {agent, task, startTime} objects, or empty array if none
 */
function findActiveTasks() {
    const edges = getEdges() || [];
    const agents = getAgents() || [];
    
    // Find all working_on edges
    const workingOnEdges = edges.filter(e => e.edge_type === 'working_on');
    
    // Map each edge to {agent, task, startTime} tuple
    const activePairs = workingOnEdges.map(edge => {
        const agent = agents.find(a => a.id === edge.source);
        const task = getNode(edge.target);  // Can be task or bug
        
        // Only include tasks that are actually in progress (not done/cancelled)
        if (agent && task && (task.status === 'pending' || task.status === 'in_progress')) {
            return { 
                agent, 
                task, 
                startTime: edge.created_at  // Use edge creation time, not task update time
            };
        }
        return null;
    }).filter(pair => pair !== null);
    
    // If no working_on edges found, fall back to in_progress tasks
    if (activePairs.length === 0) {
        const tasks = getTasks() || [];
        const bugs = getBugs() || [];
        
        // Filter out tasks that were recently unlinked from working_on edges.
        // This prevents "ghost" tasks from appearing when the edge is removed
        // but the task status update hasn't propagated yet.
        const inProgressTasks = [
            ...tasks.filter(t => t.status === 'in_progress'),
            ...bugs.filter(b => b.status === 'in_progress')
        ].filter(task => !isRecentlyUnlinked(task.id));
        
        // Return tasks without agent info (for backwards compatibility)
        // Use task.updated_at as fallback when no edge exists
        return inProgressTasks.map(task => ({ 
            agent: null, 
            task,
            startTime: task.updated_at 
        }));
    }
    
    return activePairs;
}

/**
 * Update the pane display
 * @param {HTMLElement} pane - The pane element
 */
function updatePane(pane) {
    const activePairs = findActiveTasks();
    
    if (activePairs.length === 0) {
        // No active tasks - show placeholder
        pane.classList.remove('has-task');
        pane.classList.add('no-task');
        pane.innerHTML = `
            <div class="active-task-pane-header">
                <span>Active Tasks</span>
            </div>
            <div class="active-task-placeholder">No tasks in progress</div>
        `;
        return;
    }
    
    // Active tasks found - show list
    pane.classList.remove('no-task');
    pane.classList.add('has-task');
    
    let html = `
        <div class="active-task-pane-header">
            <span>Active Tasks (${activePairs.length})</span>
        </div>
        <div class="active-task-list">
    `;
    
    for (const { agent, task, startTime } of activePairs) {
        const elapsed = calculateElapsed(startTime);
        const durationText = formatDuration(elapsed);
        const agentName = agent ? (agent.title || agent.id) : 'Unknown';
        const agentDisplay = agent ? `<div class="active-task-agent">${escapeHtml(agentName)}</div>` : '';
        
        // Determine type label based on task.type
        let typeLabel = '';
        if (task.type === 'agent') {
            typeLabel = '🤖 Worker';
        } else if (task.type === 'bug') {
            typeLabel = `Bug: ${task.id}`;
        } else {
            typeLabel = task.id;
        }
        
        html += `
            <div class="active-task-item">
                ${agentDisplay}
                <div class="active-task-id" data-task-id="${task.id}" title="Click to view task">
                    ${typeLabel}
                </div>
                <div class="active-task-title" title="${task.title}">
                    ${escapeHtml(task.short_name || task.title)}
                </div>
                <div class="active-task-timer" data-start="${startTime}">
                    ${durationText}
                </div>
            </div>
        `;
    }
    
    html += `</div>`;
    pane.innerHTML = html;
    
    // Make task IDs clickable to select the node
    const idElements = pane.querySelectorAll('.active-task-id');
    idElements.forEach(el => {
        el.addEventListener('click', () => {
            const taskId = el.getAttribute('data-task-id');
            // Import dynamically to avoid circular dependencies
            import('../state.js').then(({ viewNodeOnGraph }) => {
                viewNodeOnGraph(taskId);
            });
        });
    });
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
 * Start the timer interval for live mode
 * @param {HTMLElement} pane - The pane element
 * @returns {number|null} Interval ID, or null if not started
 */
function startTimer(pane) {
    const mode = getMode();
    
    // Only run timer in live (websocket) mode
    if (mode !== ConnectionMode.WEBSOCKET) {
        return null;
    }
    
    // Update timers every second
    const intervalId = setInterval(() => {
        const timerEls = pane.querySelectorAll('.active-task-timer');
        
        if (timerEls.length === 0) {
            // No tasks - stop timer
            clearInterval(intervalId);
            return;
        }
        
        timerEls.forEach(timerEl => {
            const startTime = timerEl.getAttribute('data-start');
            if (startTime) {
                const elapsed = calculateElapsed(startTime);
                timerEl.textContent = formatDuration(elapsed);
            }
        });
    }, 1000);
    
    return intervalId;
}

/**
 * Create the active task pane element
 * @returns {HTMLElement} The pane element
 */
export function createActiveTaskPane() {
    const pane = document.createElement('div');
    pane.className = 'active-task-pane no-task';
    pane.id = 'active-task-pane';
    
    // Track timer interval ID for cleanup
    let timerId = null;
    
    // Update function that restarts timer
    const update = () => {
        if (timerId !== null) {
            clearInterval(timerId);
            timerId = null;
        }
        updatePane(pane);
        timerId = startTimer(pane);
    };
    
    // Subscribe to state changes that affect active tasks
    subscribe('entities.tasks', update);
    subscribe('entities.bugs', update);
    subscribe('entities.agents', update);  // Agent info changes
    subscribe('edges', update);            // working_on edges added/removed
    subscribe('mode', update);
    
    // Initialize with current state
    update();
    
    return pane;
}

/**
 * Mount the active task pane to a container
 * @param {HTMLElement|string} target - Target container element or selector
 * @returns {HTMLElement|null} The pane element, or null if target not found
 */
export function mountActiveTaskPane(target) {
    const container = typeof target === 'string'
        ? document.querySelector(target)
        : target;
    
    if (!container) {
        console.warn('Active task pane: target container not found');
        return null;
    }
    
    const pane = createActiveTaskPane();
    container.appendChild(pane);
    
    return pane;
}
