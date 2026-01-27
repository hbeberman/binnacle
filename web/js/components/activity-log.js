/**
 * Activity Log Component
 * 
 * Displays activity log with dual view (entity changes + action logs),
 * entry rendering with owner display, and container structure.
 */

/**
 * Simple HTML escaping
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
 * Format timestamp as relative time
 * @param {string} isoString - ISO timestamp
 * @returns {string} Relative time (e.g., "2m ago", "5h ago")
 */
function formatRelativeTime(isoString) {
    if (!isoString) return 'N/A';
    try {
        const date = new Date(isoString);
        const now = new Date();
        const diffMs = now - date;
        const diffSec = Math.floor(diffMs / 1000);
        const diffMin = Math.floor(diffSec / 60);
        const diffHour = Math.floor(diffMin / 60);
        const diffDay = Math.floor(diffHour / 24);
        
        if (diffDay > 0) {
            return `${diffDay}d ago`;
        } else if (diffHour > 0) {
            return `${diffHour}h ago`;
        } else if (diffMin > 0) {
            return `${diffMin}m ago`;
        } else {
            return `${diffSec}s ago`;
        }
    } catch (e) {
        return isoString;
    }
}

/**
 * Get color class for action status
 * @param {boolean} success - Whether action succeeded
 * @returns {string} CSS class name
 */
function getStatusColorClass(success) {
    return success ? 'log-status-success' : 'log-status-failure';
}

/**
 * Render owner badge (user or agent)
 * @param {string} user - Username or agent ID
 * @returns {string} HTML for owner badge
 */
function renderOwnerBadge(user) {
    if (!user) return '<span class="owner-badge unknown">Unknown</span>';
    
    // Check if it's an agent ID (starts with bn-)
    const isAgent = user.startsWith('bn-');
    const icon = isAgent ? '🤖' : '👤';
    const className = isAgent ? 'owner-badge agent' : 'owner-badge user';
    
    return `<span class="${className}">${icon} ${escapeHtml(user)}</span>`;
}

/**
 * Render a single log entry
 * @param {Object} entry - Log entry object
 * @returns {string} HTML for the entry
 */
function renderLogEntry(entry) {
    const timestamp = formatRelativeTime(entry.timestamp);
    const statusClass = getStatusColorClass(entry.success);
    const ownerBadge = renderOwnerBadge(entry.user);
    const command = escapeHtml(entry.command || 'unknown');
    const exitCode = entry.exit_code !== undefined ? entry.exit_code : '?';
    
    return `
        <div class="log-entry ${statusClass}">
            <div class="log-entry-header">
                <span class="log-timestamp">${timestamp}</span>
                ${ownerBadge}
                <span class="log-command">${command}</span>
                <span class="log-exit-code">Exit: ${exitCode}</span>
            </div>
            ${entry.args && entry.args.length > 0 ? `
            <div class="log-entry-details">
                <span class="log-args">${escapeHtml(entry.args.join(' '))}</span>
            </div>
            ` : ''}
        </div>
    `;
}

/**
 * Fetch log entries from API
 * @param {number} limit - Maximum entries to fetch
 * @param {number} offset - Offset for pagination
 * @returns {Promise<Object>} Log response with entries and total
 */
async function fetchLogs(limit = 50, offset = 0) {
    try {
        const params = new URLSearchParams({
            limit: limit.toString(),
            offset: offset.toString()
        });
        
        const response = await fetch(`/api/log?${params}`);
        if (!response.ok) {
            throw new Error(`HTTP ${response.status}`);
        }
        
        const data = await response.json();
        return {
            entries: data.entries || [],
            total: data.total || 0
        };
    } catch (error) {
        console.error('Failed to fetch logs:', error);
        return { entries: [], total: 0, error: error.message };
    }
}

/**
 * Update the log display
 * @param {HTMLElement} container - Container element
 * @param {number} offset - Current pagination offset
 */
async function updateLogDisplay(container, offset = 0) {
    const entriesContainer = container.querySelector('.log-entries');
    const loadingEl = container.querySelector('.log-loading');
    const errorEl = container.querySelector('.log-error');
    const emptyEl = container.querySelector('.log-empty');
    const paginationEl = container.querySelector('.log-pagination');
    
    // Show loading state
    loadingEl.style.display = 'block';
    errorEl.style.display = 'none';
    emptyEl.style.display = 'none';
    entriesContainer.innerHTML = '';
    
    const result = await fetchLogs(50, offset);
    
    loadingEl.style.display = 'none';
    
    if (result.error) {
        errorEl.textContent = `Error loading logs: ${result.error}`;
        errorEl.style.display = 'block';
        return;
    }
    
    if (result.entries.length === 0 && offset === 0) {
        emptyEl.style.display = 'block';
        return;
    }
    
    // Render entries
    const entriesHTML = result.entries.map(renderLogEntry).join('');
    entriesContainer.innerHTML = entriesHTML;
    
    // Update pagination
    const currentPage = Math.floor(offset / 50) + 1;
    const totalPages = Math.ceil(result.total / 50);
    const hasPrev = offset > 0;
    const hasNext = offset + 50 < result.total;
    
    paginationEl.innerHTML = `
        <button class="pagination-btn" data-action="prev" ${!hasPrev ? 'disabled' : ''}>← Previous</button>
        <span class="pagination-info">Page ${currentPage} of ${totalPages} (${result.total} total)</span>
        <button class="pagination-btn" data-action="next" ${!hasNext ? 'disabled' : ''}>Next →</button>
    `;
    
    // Store current offset for pagination handlers
    container.dataset.currentOffset = offset.toString();
}

/**
 * Create the activity log component
 * @returns {HTMLElement} The activity log element
 */
export function createActivityLog() {
    const container = document.createElement('div');
    container.className = 'activity-log-container';
    
    container.innerHTML = `
        <div class="activity-log-header">
            <h3>Command History</h3>
            <div class="activity-log-controls">
                <button class="btn-refresh" title="Refresh log">🔄 Refresh</button>
            </div>
        </div>
        
        <div class="log-loading">Loading logs...</div>
        <div class="log-error" style="display: none; color: var(--error-color); padding: 1rem;"></div>
        <div class="log-empty" style="display: none; padding: 1rem; color: var(--text-secondary);">
            No log entries found. Run some commands to see them here.
        </div>
        
        <div class="log-entries"></div>
        
        <div class="log-pagination" style="display: flex; justify-content: space-between; align-items: center; padding: 1rem; border-top: 1px solid var(--border-color);"></div>
    `;
    
    // Initial load
    updateLogDisplay(container, 0);
    
    // Refresh button handler
    container.querySelector('.btn-refresh').addEventListener('click', () => {
        const offset = parseInt(container.dataset.currentOffset || '0', 10);
        updateLogDisplay(container, offset);
    });
    
    // Pagination handlers
    container.addEventListener('click', (e) => {
        const btn = e.target.closest('.pagination-btn');
        if (!btn || btn.disabled) return;
        
        const action = btn.dataset.action;
        const currentOffset = parseInt(container.dataset.currentOffset || '0', 10);
        
        if (action === 'prev') {
            updateLogDisplay(container, Math.max(0, currentOffset - 50));
        } else if (action === 'next') {
            updateLogDisplay(container, currentOffset + 50);
        }
    });
    
    return container;
}

/**
 * Mount the activity log to a container
 * @param {HTMLElement|string} target - Target container element or selector
 * @returns {HTMLElement|null} The activity log element, or null if target not found
 */
export function mountActivityLog(target) {
    const targetEl = typeof target === 'string' 
        ? document.querySelector(target)
        : target;
    
    if (!targetEl) {
        console.warn('Activity log: target container not found');
        return null;
    }
    
    const log = createActivityLog();
    targetEl.appendChild(log);
    
    return log;
}
