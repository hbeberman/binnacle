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
    getMode,
    ConnectionMode
} from '../state.js';

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
 * Find the active (in_progress) task
 * @returns {Object|null} The in_progress task, or null if none
 */
function findActiveTask() {
    const tasks = getTasks() || [];
    // Find first in_progress task (should only be one per agent session)
    return tasks.find(t => t.status === 'in_progress') || null;
}

/**
 * Update the pane display
 * @param {HTMLElement} pane - The pane element
 */
function updatePane(pane) {
    const activeTask = findActiveTask();
    
    if (!activeTask) {
        // No active task - show placeholder
        pane.classList.remove('has-task');
        pane.classList.add('no-task');
        pane.innerHTML = `
            <div class="active-task-pane-header">
                <span>Active Task</span>
            </div>
            <div class="active-task-placeholder">No task in progress</div>
        `;
        return;
    }
    
    // Active task found - show details
    pane.classList.remove('no-task');
    pane.classList.add('has-task');
    
    const elapsed = calculateElapsed(activeTask.updated_at);
    const durationText = formatDuration(elapsed);
    
    pane.innerHTML = `
        <div class="active-task-pane-header">
            <span>Active Task</span>
        </div>
        <div class="active-task-id" data-task-id="${activeTask.id}" title="Click to view task">
            ${activeTask.id}
        </div>
        <div class="active-task-title" title="${activeTask.title}">
            ${activeTask.short_name || activeTask.title}
        </div>
        <div class="active-task-timer" data-start="${activeTask.updated_at}">
            ${durationText}
        </div>
    `;
    
    // Make task ID clickable to select the node
    const idElement = pane.querySelector('.active-task-id');
    idElement.addEventListener('click', () => {
        // Import dynamically to avoid circular dependencies
        import('../state.js').then(({ setSelectedNode }) => {
            setSelectedNode(activeTask.id);
        });
    });
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
    
    // Update timer every second
    const intervalId = setInterval(() => {
        const timerEl = pane.querySelector('.active-task-timer');
        const startTime = timerEl?.getAttribute('data-start');
        
        if (!timerEl || !startTime) {
            // Task disappeared - stop timer
            clearInterval(intervalId);
            return;
        }
        
        const elapsed = calculateElapsed(startTime);
        timerEl.textContent = formatDuration(elapsed);
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
    
    // Subscribe to state changes that affect active task
    subscribe('entities.tasks', update);
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
