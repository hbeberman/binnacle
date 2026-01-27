/**
 * Activity Log Component
 * 
 * Displays activity log with dual view (entity changes + action logs),
 * entry rendering with owner display, and container structure.
 */

import { subscribe } from '../state.js';
import { showNodeDetailModal } from './node-detail-modal.js';

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
    
    // Make agent IDs clickable
    if (isAgent) {
        return `<span class="${className} clickable-entity-id" data-entity-id="${escapeHtml(user)}" title="Click to view ${escapeHtml(user)}">${icon} ${escapeHtml(user)}</span>`;
    }
    
    return `<span class="${className}">${icon} ${escapeHtml(user)}</span>`;
}

/**
 * Regex to match binnacle entity IDs
 */
const ENTITY_ID_PATTERN = /\b(bn[a-z]?-[a-f0-9]{4})\b/gi;

/**
 * Make entity IDs in text clickable
 * @param {string} text - Text that may contain entity IDs
 * @returns {string} HTML with clickable entity IDs
 */
function makeEntityIdsClickable(text) {
    if (!text || !ENTITY_ID_PATTERN.test(text)) {
        return escapeHtml(text);
    }
    
    // Reset regex
    ENTITY_ID_PATTERN.lastIndex = 0;
    
    let result = '';
    let lastIndex = 0;
    let match;
    
    while ((match = ENTITY_ID_PATTERN.exec(text)) !== null) {
        // Add escaped text before match
        result += escapeHtml(text.slice(lastIndex, match.index));
        
        // Add clickable entity ID
        const entityId = match[0];
        result += `<span class="clickable-entity-id" data-entity-id="${entityId}" title="Click to view ${entityId}">${entityId}</span>`;
        
        lastIndex = ENTITY_ID_PATTERN.lastIndex;
    }
    
    // Add remaining text
    result += escapeHtml(text.slice(lastIndex));
    
    return result;
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
    
    // Make entity IDs clickable in args
    const argsWithClickableIds = entry.args && entry.args.length > 0
        ? makeEntityIdsClickable(entry.args.join(' '))
        : '';
    
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
                <span class="log-args">${argsWithClickableIds}</span>
            </div>
            ` : ''}
        </div>
    `;
}

/**
 * Fetch distinct log owners from API
 * @returns {Promise<Array>} List of owner usernames/agent IDs
 */
async function fetchLogOwners() {
    try {
        const response = await fetch('/api/log/owners');
        if (!response.ok) {
            throw new Error(`HTTP ${response.status}`);
        }
        
        const data = await response.json();
        return data.owners || [];
    } catch (error) {
        console.error('Failed to fetch log owners:', error);
        return [];
    }
}

/**
 * Populate owner dropdown with distinct owners
 * @param {HTMLSelectElement} select - The owner select element
 */
async function populateOwnerDropdown(select) {
    const owners = await fetchLogOwners();
    
    // Keep "All Owners" option
    select.innerHTML = '<option value="all">All Owners</option>';
    
    // Add each owner as an option
    owners.forEach(owner => {
        const option = document.createElement('option');
        option.value = owner;
        option.textContent = owner;
        select.appendChild(option);
    });
}

/**
 * Fetch log entries from API
 * @param {number} limit - Maximum entries to fetch
 * @param {number} offset - Offset for pagination
 * @param {Object} filters - Filter options
 * @returns {Promise<Object>} Log response with entries and total
 */
async function fetchLogs(limit = 50, offset = 0, filters = {}) {
    try {
        const params = new URLSearchParams({
            limit: limit.toString(),
            offset: offset.toString()
        });
        
        // Add filter params if present
        if (filters.view && filters.view !== 'all') {
            params.set('view', filters.view);
        }
        if (filters.type && filters.type !== 'all') {
            params.set('type', filters.type);
        }
        if (filters.timeRange && filters.timeRange !== 'all') {
            params.set('time_range', filters.timeRange);
        }
        if (filters.owner && filters.owner !== 'all') {
            params.set('user', filters.owner);
        }
        if (filters.search) {
            params.set('search', filters.search);
        }
        
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
 * Get current filters from the filter bar
 * @param {HTMLElement} container - Container element
 * @returns {Object} Current filter values
 */
function getCurrentFilters(container) {
    const filterBar = container.querySelector('.log-filter-bar');
    if (!filterBar) return {};
    
    return {
        view: filterBar.querySelector('[name="view"]')?.value || 'all',
        type: filterBar.querySelector('[name="type"]')?.value || 'all',
        timeRange: filterBar.querySelector('[name="time-range"]')?.value || 'all',
        owner: filterBar.querySelector('[name="owner"]')?.value || 'all',
        search: filterBar.querySelector('[name="search"]')?.value || ''
    };
}

/**
 * Check if any filters are active
 * @param {Object} filters - Filter object
 * @returns {boolean} True if any filters are active
 */
function hasActiveFilters(filters) {
    return filters.view !== 'all' || 
           filters.type !== 'all' || 
           filters.timeRange !== 'all' || 
           filters.owner !== 'all' || 
           filters.search !== '';
}

/**
 * Update the log display
 * @param {HTMLElement} container - Container element
 * @param {number} offset - Current pagination offset
 * @param {boolean} append - If true, append to existing entries instead of replacing
 */
async function updateLogDisplay(container, offset = 0, append = false) {
    const entriesContainer = container.querySelector('.log-entries');
    const loadingEl = container.querySelector('.log-loading');
    const errorEl = container.querySelector('.log-error');
    const emptyEl = container.querySelector('.log-empty');
    const loadMoreBtn = container.querySelector('.log-load-more-btn');
    const loadingMoreEl = container.querySelector('.log-loading-more');
    
    // Get current filters
    const filters = getCurrentFilters(container);
    
    // Update clear button state
    const clearBtn = container.querySelector('.filter-clear-btn');
    if (clearBtn) {
        clearBtn.disabled = !hasActiveFilters(filters);
    }
    
    // Show appropriate loading state
    if (append) {
        loadingMoreEl.style.display = 'block';
        if (loadMoreBtn) loadMoreBtn.style.display = 'none';
    } else {
        loadingEl.style.display = 'block';
        errorEl.style.display = 'none';
        emptyEl.style.display = 'none';
        entriesContainer.innerHTML = '';
    }
    
    const result = await fetchLogs(50, offset, filters);
    
    // Hide loading states
    loadingEl.style.display = 'none';
    loadingMoreEl.style.display = 'none';
    
    if (result.error) {
        errorEl.textContent = `Error loading logs: ${result.error}`;
        errorEl.style.display = 'block';
        return;
    }
    
    if (result.entries.length === 0 && offset === 0) {
        emptyEl.style.display = 'block';
        if (loadMoreBtn) loadMoreBtn.style.display = 'none';
        return;
    }
    
    // Render entries
    const entriesHTML = result.entries.map(renderLogEntry).join('');
    if (append) {
        entriesContainer.insertAdjacentHTML('beforeend', entriesHTML);
    } else {
        entriesContainer.innerHTML = entriesHTML;
    }
    
    // Update load more button visibility
    const hasMore = offset + result.entries.length < result.total;
    if (loadMoreBtn) {
        loadMoreBtn.style.display = hasMore ? 'block' : 'none';
        const loadedCount = offset + result.entries.length;
        loadMoreBtn.textContent = `Load More (${loadedCount} of ${result.total})`;
    }
    
    // Store current offset for load more handler
    container.dataset.currentOffset = (offset + result.entries.length).toString();
    container.dataset.totalEntries = result.total.toString();
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
        
        <div class="log-filter-bar">
            <div class="filter-group">
                <label class="filter-label" for="filter-view">View:</label>
                <select class="filter-select" name="view" id="filter-view">
                    <option value="all">All</option>
                    <option value="entity">Entity Changes</option>
                    <option value="action">Action Logs</option>
                </select>
            </div>
            
            <div class="filter-group">
                <label class="filter-label" for="filter-type">Type:</label>
                <select class="filter-select" name="type" id="filter-type">
                    <option value="all">All Types</option>
                    <option value="task">Task</option>
                    <option value="bug">Bug</option>
                    <option value="idea">Idea</option>
                    <option value="milestone">Milestone</option>
                    <option value="queue">Queue</option>
                    <option value="doc">Doc</option>
                </select>
            </div>
            
            <div class="filter-group">
                <label class="filter-label" for="filter-time">Time:</label>
                <select class="filter-select" name="time-range" id="filter-time">
                    <option value="all">All Time</option>
                    <option value="1h">Last Hour</option>
                    <option value="24h">Last 24 Hours</option>
                    <option value="7d">Last 7 Days</option>
                    <option value="30d">Last 30 Days</option>
                </select>
            </div>
            
            <div class="filter-group">
                <label class="filter-label" for="filter-owner">Owner:</label>
                <select class="filter-select" name="owner" id="filter-owner">
                    <option value="all">All Owners</option>
                    <!-- Dynamically populated -->
                </select>
            </div>
            
            <input 
                type="text" 
                class="filter-search" 
                name="search" 
                placeholder="🔍 Search logs..." 
                autocomplete="off"
            />
            
            <button class="filter-clear-btn" disabled>Clear Filters</button>
        </div>
        
        <div class="log-loading">Loading logs...</div>
        <div class="log-error" style="display: none; color: var(--error-color); padding: 1rem;"></div>
        <div class="log-empty" style="display: none; padding: 1rem; color: var(--text-secondary);">
            No log entries found. Run some commands to see them here.
        </div>
        
        <div class="log-entries"></div>
        
        <div class="log-loading-more" style="display: none; text-align: center; padding: 1rem; color: var(--text-secondary);">Loading more entries...</div>
        
        <button class="log-load-more-btn" style="display: none; width: 100%; padding: 1rem; margin-top: 1rem; background: var(--bg-tertiary); color: var(--text-primary); border: 1px solid var(--border-color); border-radius: 4px; cursor: pointer; font-size: 0.9rem;">
            Load More
        </button>
    `;
    
    // Populate owner dropdown with distinct owners
    const ownerSelect = container.querySelector('[name="owner"]');
    populateOwnerDropdown(ownerSelect);
    
    // Initial load
    updateLogDisplay(container, 0);
    
    // Refresh button handler
    container.querySelector('.btn-refresh').addEventListener('click', () => {
        // Reset to top on refresh
        updateLogDisplay(container, 0);
    });
    
    // Filter change handlers
    const filterBar = container.querySelector('.log-filter-bar');
    filterBar.addEventListener('change', () => {
        // Reset to first page when filters change
        updateLogDisplay(container, 0);
    });
    
    // Search input handler (debounced)
    let searchTimeout;
    const searchInput = container.querySelector('[name="search"]');
    searchInput.addEventListener('input', () => {
        clearTimeout(searchTimeout);
        searchTimeout = setTimeout(() => {
            updateLogDisplay(container, 0);
        }, 300); // 300ms debounce
    });
    
    // Clear filters button handler
    container.querySelector('.filter-clear-btn').addEventListener('click', () => {
        // Reset all filters to default
        container.querySelector('[name="view"]').value = 'all';
        container.querySelector('[name="type"]').value = 'all';
        container.querySelector('[name="time-range"]').value = 'all';
        container.querySelector('[name="owner"]').value = 'all';
        container.querySelector('[name="search"]').value = '';
        
        // Reload with cleared filters
        updateLogDisplay(container, 0);
    });
    
    // Load more button handler
    container.querySelector('.log-load-more-btn').addEventListener('click', () => {
        const currentOffset = parseInt(container.dataset.currentOffset || '0', 10);
        updateLogDisplay(container, currentOffset, true);
    });
    
    // Entity ID click handlers
    container.addEventListener('click', (e) => {
        const clickableId = e.target.closest('.clickable-entity-id');
        if (!clickableId) return;
        
        e.preventDefault();
        e.stopPropagation();
        
        const entityId = clickableId.dataset.entityId;
        if (entityId) {
            showNodeDetailModal(entityId);
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
    
    // Subscribe to log state changes for real-time updates
    subscribe('log', () => {
        // When log state changes (e.g., from WebSocket sync), refresh from top
        // New entries appear at top, so resetting makes sense
        updateLogDisplay(log, 0);
    });
    
    // Subscribe to view changes to refresh when switching to log view
    subscribe('ui.currentView', (view) => {
        if (view === 'log') {
            // Refresh log from top when switching to log view
            updateLogDisplay(log, 0);
        }
    });
    
    return log;
}
