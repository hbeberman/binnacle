/**
 * Available Work Pane Component
 * 
 * Displays count of claimable work items in the header:
 * - Total count (large number)
 * - Breakdown by type (tasks, bugs)
 * 
 * Updates automatically when entity data changes.
 */

import { 
    subscribe, 
    getReady,
    getBugs
} from '../state.js';

/**
 * Create emoji+count badge element
 * @param {string} emoji - Emoji to display
 * @param {number} count - Count to display
 * @param {string} type - Type for styling (task, bug, idea)
 * @param {string} label - Tooltip label
 * @returns {HTMLElement} Badge element
 */
function createEmojiCountBadge(emoji, count, type, label) {
    const badge = document.createElement('span');
    badge.className = `emoji-count-badge type-${type}`;
    badge.title = label;
    badge.innerHTML = `<span class="emoji">${emoji}</span><span class="count">${count}</span>`;
    return badge;
}

/**
 * Calculate work counts from current state
 * @returns {Object} Counts object { total, readyTasks, openBugs }
 */
function calculateWorkCounts() {
    // Get ready items (excluding bugs and ideas - they show separately)
    const ready = getReady() || [];
    const readyTasks = ready.filter(t => t.type !== 'bug' && t.type !== 'idea');
    const readyTaskCount = readyTasks.length;
    
    // Count open bugs (not done, not cancelled)
    const allBugs = getBugs() || [];
    const openBugs = allBugs.filter(b => 
        b.status !== 'done' && 
        b.status !== 'cancelled'
    );
    const openBugCount = openBugs.length;
    
    // Ideas are excluded - they require human review/refinement before becoming actionable
    
    const total = readyTaskCount + openBugCount;
    
    return {
        total,
        readyTasks: readyTaskCount,
        openBugs: openBugCount
    };
}

/**
 * Update the pane display
 * @param {HTMLElement} pane - The pane element
 */
function updatePane(pane) {
    const countEl = pane.querySelector('.available-work-count');
    const breakdownEl = pane.querySelector('.available-work-breakdown');
    
    const counts = calculateWorkCounts();
    
    countEl.textContent = counts.total;
    
    // Update styling based on whether there's work
    pane.classList.remove('has-work', 'no-work');
    pane.classList.add(counts.total > 0 ? 'has-work' : 'no-work');
    
    // Show breakdown using emoji badges
    breakdownEl.innerHTML = '';
    if (counts.readyTasks > 0) {
        const label = `${counts.readyTasks} ready task${counts.readyTasks !== 1 ? 's' : ''}`;
        breakdownEl.appendChild(createEmojiCountBadge('📋', counts.readyTasks, 'task', label));
    }
    if (counts.openBugs > 0) {
        const label = `${counts.openBugs} open bug${counts.openBugs !== 1 ? 's' : ''}`;
        breakdownEl.appendChild(createEmojiCountBadge('🐛', counts.openBugs, 'bug', label));
    }
}

/**
 * Create the available work pane element
 * @returns {HTMLElement} The pane element
 */
export function createAvailableWorkPane() {
    const pane = document.createElement('div');
    pane.className = 'available-work-pane no-work';
    pane.id = 'available-work-pane';
    
    pane.innerHTML = `
        <div class="available-work-pane-header">
            <span>Available Work</span>
        </div>
        <div class="available-work-count">0</div>
        <div class="available-work-breakdown"></div>
    `;
    
    // Subscribe to state changes that affect work count
    subscribe('ready', () => updatePane(pane));
    subscribe('entities.bugs', () => updatePane(pane));
    
    // Initialize with current state
    updatePane(pane);
    
    return pane;
}

/**
 * Mount the available work pane to a container
 * @param {HTMLElement|string} target - Target container element or selector
 * @returns {HTMLElement|null} The pane element, or null if target not found
 */
export function mountAvailableWorkPane(target) {
    const container = typeof target === 'string'
        ? document.querySelector(target)
        : target;
    
    if (!container) {
        console.warn('Available work pane: target container not found');
        return null;
    }
    
    const pane = createAvailableWorkPane();
    container.appendChild(pane);
    
    return pane;
}
