/**
 * Agent Cards Component
 * 
 * Displays agents in spacious, informational cards showing all known agent information.
 */

import { 
    subscribe, 
    getAgents,
    setSelectedNode
} from '../state.js';
import { createClickableId } from '../utils/clickable-ids.js';
import { showNodeDetailModal } from './node-detail-modal.js';

/**
 * Get status badge configuration for an agent
 * @param {string} status - Agent status (active, idle, stale)
 * @returns {Object} Badge config { emoji, className, label }
 */
function getStatusBadge(status) {
    const statusLower = (status || 'unknown').toLowerCase();
    
    switch (statusLower) {
        case 'active':
            return { emoji: '🟢', className: 'status-active', label: 'Active' };
        case 'idle':
            return { emoji: '🟡', className: 'status-idle', label: 'Idle' };
        case 'stale':
            return { emoji: '🔴', className: 'status-stale', label: 'Stale' };
        default:
            return { emoji: '⚪', className: 'status-unknown', label: 'Unknown' };
    }
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
 * Format a date/time for display
 * @param {string} isoString - ISO date string
 * @returns {string} Formatted date/time
 */
function formatDateTime(isoString) {
    if (!isoString) return 'N/A';
    try {
        const date = new Date(isoString);
        return date.toLocaleString();
    } catch (e) {
        return isoString;
    }
}

/**
 * Calculate elapsed time from a start time
 * @param {string} startTime - ISO start time string
 * @returns {string} Formatted elapsed time
 */
function formatElapsedTime(startTime) {
    if (!startTime) return 'N/A';
    try {
        const start = new Date(startTime);
        const now = new Date();
        const diffMs = now - start;
        const diffSec = Math.floor(diffMs / 1000);
        const diffMin = Math.floor(diffSec / 60);
        const diffHour = Math.floor(diffMin / 60);
        const diffDay = Math.floor(diffHour / 24);
        
        if (diffDay > 0) {
            return `${diffDay}d ${diffHour % 24}h`;
        } else if (diffHour > 0) {
            return `${diffHour}h ${diffMin % 60}m`;
        } else if (diffMin > 0) {
            return `${diffMin}m ${diffSec % 60}s`;
        } else {
            return `${diffSec}s`;
        }
    } catch (e) {
        return 'N/A';
    }
}

/**
 * Create health warnings section for an agent card
 * @param {Object} agent - Agent entity
 * @returns {HTMLElement|null} Health warnings element or null if no warnings
 */
function createHealthWarningsSection(agent) {
    const health = agent._agent?.health || agent.health;
    const status = (agent.status || '').toLowerCase();
    
    // Only show warnings for stale or stuck agents
    if (status !== 'stale' && (!health || !health.is_stuck)) {
        return null;
    }
    
    const section = document.createElement('div');
    section.className = 'agent-card-health-warnings';
    
    // Stale agent warning
    if (status === 'stale' && health) {
        const staleWarning = document.createElement('div');
        staleWarning.className = 'agent-card-health-warning agent-card-health-stale';
        
        const idleMinutes = health.idle_minutes || 0;
        const timeStr = idleMinutes >= 60 
            ? `${Math.floor(idleMinutes / 60)}h ${idleMinutes % 60}m`
            : `${idleMinutes}m`;
        
        staleWarning.innerHTML = `
            <span class="agent-card-health-icon">⚠️</span>
            <span class="agent-card-health-text">Agent stale (${timeStr} since last activity)</span>
        `;
        section.appendChild(staleWarning);
    }
    
    // Stuck agent warning
    if (health && health.is_stuck) {
        const stuckWarning = document.createElement('div');
        stuckWarning.className = 'agent-card-health-warning agent-card-health-stuck';
        
        const idleMinutes = health.idle_minutes || 0;
        const timeStr = idleMinutes >= 60 
            ? `${Math.floor(idleMinutes / 60)}h ${idleMinutes % 60}m`
            : `${idleMinutes}m`;
        
        const taskInfo = health.stuck_task_ids && health.stuck_task_ids.length > 0
            ? ` on task${health.stuck_task_ids.length > 1 ? 's' : ''}`
            : '';
        
        stuckWarning.innerHTML = `
            <span class="agent-card-health-icon">🔒</span>
            <span class="agent-card-health-text">Agent stuck${taskInfo} (${timeStr} idle)</span>
        `;
        section.appendChild(stuckWarning);
    }
    
    return section;
}

/**
 * Create task links section for an agent card
 * @param {Array<string>} tasks - Array of task IDs
 * @returns {HTMLElement} Task section element
 */
function createTaskLinksSection(tasks) {
    const section = document.createElement('div');
    section.className = 'agent-card-tasks-section';
    
    if (!tasks || tasks.length === 0) {
        section.innerHTML = `
            <div class="agent-card-tasks-label">Working on</div>
            <div class="agent-card-no-tasks">
                <span class="agent-card-no-tasks-icon">🎯</span>
                <span class="agent-card-no-tasks-text">No active task</span>
            </div>
        `;
        return section;
    }
    
    const label = document.createElement('div');
    label.className = 'agent-card-tasks-label';
    label.textContent = 'Working on';
    section.appendChild(label);
    
    const taskList = document.createElement('div');
    taskList.className = 'agent-card-tasks-list';
    
    for (const taskId of tasks) {
        const taskItem = document.createElement('div');
        taskItem.className = 'agent-card-task-item';
        
        // Create clickable task ID
        const clickableId = createClickableId(taskId);
        
        // Override click behavior to show detail modal instead of navigating to graph
        clickableId.addEventListener('click', (e) => {
            e.preventDefault();
            e.stopPropagation();
            showNodeDetailModal(taskId);
        });
        
        taskItem.appendChild(clickableId);
        
        taskList.appendChild(taskItem);
    }
    
    section.appendChild(taskList);
    
    return section;
}

/**
 * Create an agent card element
 * @param {Object} agent - Agent entity
 * @returns {HTMLElement} Card element
 */
function createAgentCard(agent) {
    const card = document.createElement('div');
    card.className = 'agent-card';
    card.dataset.agentId = agent.id;
    
    const badge = getStatusBadge(agent.status);
    
    // Extract all available information
    const name = agent._agent?.name || agent.title || 'Unknown Agent';
    const purpose = agent._agent?.purpose || agent.description || 'No purpose specified';
    const agentId = agent.id;
    const pid = agent.pid || agent._agent?.pid || 'N/A';
    const startedAt = agent._agent?.started_at || agent.started_at;
    const shortName = agent.short_name || '';
    const tasks = agent._agent?.tasks || agent.tasks || [];
    
    card.innerHTML = `
        <div class="agent-card-header">
            <div class="agent-card-title-section">
                <h3 class="agent-card-name">${escapeHtml(name)}</h3>
                <div class="agent-card-id">${escapeHtml(agentId)}</div>
            </div>
            <div class="agent-card-status-badge ${badge.className}">
                <span>${badge.emoji}</span>
                <span>${badge.label}</span>
            </div>
        </div>
    `;
    
    // Add health warnings section (if any)
    const healthWarnings = createHealthWarningsSection(agent);
    if (healthWarnings) {
        card.appendChild(healthWarnings);
    }
    
    // Add task links section
    const taskSection = createTaskLinksSection(tasks);
    card.appendChild(taskSection);
    
    // Add info grid
    const infoGrid = document.createElement('div');
    infoGrid.className = 'agent-card-info-grid';
    infoGrid.innerHTML = `
        <div class="agent-card-info-item">
            <div class="agent-card-info-label">Process ID</div>
            <div class="agent-card-info-value">${escapeHtml(String(pid))}</div>
        </div>
        <div class="agent-card-info-item">
            <div class="agent-card-info-label">Uptime</div>
            <div class="agent-card-info-value">${formatElapsedTime(startedAt)}</div>
        </div>
        <div class="agent-card-info-item">
            <div class="agent-card-info-label">Started At</div>
            <div class="agent-card-info-value">${formatDateTime(startedAt)}</div>
        </div>
        ${shortName ? `
        <div class="agent-card-info-item">
            <div class="agent-card-info-label">Short Name</div>
            <div class="agent-card-info-value">${escapeHtml(shortName)}</div>
        </div>
        ` : `
        <div class="agent-card-info-item">
            <div class="agent-card-info-label">Status</div>
            <div class="agent-card-info-value">${badge.label}</div>
        </div>
        `}
    `;
    card.appendChild(infoGrid);
    
    // Add purpose section if present
    if (purpose !== 'No purpose specified') {
        const purposeDiv = document.createElement('div');
        purposeDiv.className = 'agent-card-purpose';
        purposeDiv.innerHTML = `
            <div class="agent-card-purpose-label">Purpose</div>
            <div class="agent-card-purpose-text">${escapeHtml(purpose)}</div>
        `;
        card.appendChild(purposeDiv);
    }
    
    // Click to select agent in graph
    card.addEventListener('click', () => {
        setSelectedNode(agent.id);
        console.log('Selected agent:', agent.id);
        // TODO: Could also switch to graph view and pan to agent
    });
    
    return card;
}

/**
 * Update the agent cards display
 * @param {HTMLElement} container - The agent cards container
 */
function updateAgentCards(container) {
    if (!container) return;
    
    const agents = getAgents() || [];
    
    // Clear existing cards
    container.innerHTML = '';
    
    if (agents.length === 0) {
        const emptyState = document.createElement('div');
        emptyState.className = 'agent-cards-empty';
        emptyState.innerHTML = `
            <div class="agent-cards-empty-icon">🤖</div>
            <div class="agent-cards-empty-text">No agents running</div>
            <div class="agent-cards-empty-hint">Agents will appear here when they start working on tasks</div>
        `;
        container.appendChild(emptyState);
        return;
    }
    
    // Sort agents: active first, then idle, then stale
    const statusOrder = { 'active': 0, 'idle': 1, 'stale': 2, 'unknown': 3 };
    const sortedAgents = [...agents].sort((a, b) => {
        const aStatus = (a.status || 'unknown').toLowerCase();
        const bStatus = (b.status || 'unknown').toLowerCase();
        const orderA = statusOrder[aStatus] ?? 3;
        const orderB = statusOrder[bStatus] ?? 3;
        
        if (orderA !== orderB) {
            return orderA - orderB;
        }
        
        // Secondary sort by started time (newer first)
        const aTime = a._agent?.started_at || a.started_at;
        const bTime = b._agent?.started_at || b.started_at;
        if (aTime && bTime) {
            return new Date(bTime) - new Date(aTime);
        }
        
        // Tertiary sort by name
        const nameA = a._agent?.name || a.title || a.id;
        const nameB = b._agent?.name || b.title || b.id;
        return nameA.localeCompare(nameB);
    });
    
    // Create cards
    for (const agent of sortedAgents) {
        container.appendChild(createAgentCard(agent));
    }
}

/**
 * Initialize the agent cards view
 * @param {HTMLElement|string} container - Container element or selector
 * @returns {HTMLElement|null} The container element, or null if not found
 */
export function initializeAgentCards(container) {
    const element = typeof container === 'string'
        ? document.querySelector(container)
        : container;
    
    if (!element) {
        console.warn('Agent cards: container not found');
        return null;
    }
    
    // Add the cards container class
    element.classList.add('agent-cards-container');
    
    // Subscribe to agent changes
    subscribe('entities.agents', () => updateAgentCards(element));
    
    // Initial render
    updateAgentCards(element);
    
    return element;
}
