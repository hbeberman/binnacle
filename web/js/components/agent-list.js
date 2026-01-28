/**
 * Agent List Component
 * 
 * Displays agents in the sidebar with status badges (active/idle/stale).
 * Click to focus agent in graph.
 */

import { 
    subscribe, 
    getAgents,
    setSelectedNode
} from '../state.js';

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
 * Create an agent list item element
 * @param {Object} agent - Agent entity
 * @returns {HTMLElement} List item element
 */
function createAgentItem(agent) {
    const item = document.createElement('div');
    item.className = 'agent-item';
    item.dataset.agentId = agent.id;
    
    const badge = getStatusBadge(agent.status);
    
    // Agent name (or purpose if available)
    const name = agent._agent?.purpose || agent._agent?.name || agent.title || agent.id;
    
    // Time info (optional - show if available)
    const timeInfo = agent._agent?.started_at 
        ? `Started: ${new Date(agent._agent.started_at).toLocaleTimeString()}`
        : '';
    
    item.innerHTML = `
        <div class="agent-item-status ${badge.className}" title="${badge.label}">
            ${badge.emoji}
        </div>
        <div class="agent-item-content">
            <div class="agent-item-name">${escapeHtml(name)}</div>
            ${timeInfo ? `<div class="agent-item-time">${escapeHtml(timeInfo)}</div>` : ''}
        </div>
    `;
    
    // Click to focus agent in graph
    item.addEventListener('click', () => {
        setSelectedNode(agent.id);
        
        // TODO: Pan to node in graph (requires graph camera integration)
        // For now, just select it - the graph will highlight it
    });
    
    return item;
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
 * Update the agent list display
 * @param {HTMLElement} container - The agent list container
 */
function updateAgentList(container) {
    if (!container) return;
    
    const agents = getAgents() || [];
    
    // Clear existing items
    container.innerHTML = '';
    
    if (agents.length === 0) {
        const emptyState = document.createElement('div');
        emptyState.className = 'agent-list-empty';
        emptyState.textContent = 'No agents';
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
        
        // Secondary sort by name
        const nameA = a._agent?.name || a.title || a.id;
        const nameB = b._agent?.name || b.title || b.id;
        return nameA.localeCompare(nameB);
    });
    
    // Create list items
    for (const agent of sortedAgents) {
        container.appendChild(createAgentItem(agent));
    }
}

/**
 * Initialize the agent list in the sidebar
 * @param {HTMLElement|string} container - Container element or selector
 * @returns {HTMLElement|null} The container element, or null if not found
 */
export function initializeAgentList(container) {
    const element = typeof container === 'string'
        ? document.querySelector(container)
        : container;
    
    if (!element) {
        console.warn('Agent list: container not found');
        return null;
    }
    
    // Subscribe to agent changes
    subscribe('entities.agents', () => updateAgentList(element));
    
    // Initial render
    updateAgentList(element);
    
    return element;
}
