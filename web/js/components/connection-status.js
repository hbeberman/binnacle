/**
 * Connection Status Indicator Component
 * 
 * Displays WebSocket connection status in the header:
 * - Green dot: Connected
 * - Red dot: Disconnected/Error
 * - Yellow dot: Connecting/Reconnecting
 * 
 * Updates automatically on connection state changes.
 */

import { 
    subscribe, 
    getConnectionStatus,
    ConnectionStatus 
} from '../state.js';

/**
 * Create the connection status indicator element
 * @returns {HTMLElement} The indicator element
 */
export function createConnectionStatusIndicator() {
    const indicator = document.createElement('div');
    indicator.className = 'connection-status-indicator';
    indicator.id = 'connection-status';
    
    indicator.innerHTML = `
        <span class="connection-status-dot"></span>
        <span class="connection-status-text">Disconnected</span>
    `;
    
    // Subscribe to connection status changes
    subscribe('connectionStatus', (status) => {
        updateIndicator(indicator, status);
    });
    
    // Initialize with current state
    updateIndicator(indicator, getConnectionStatus());
    
    return indicator;
}

/**
 * Update the indicator display based on connection status
 * @param {HTMLElement} indicator - The indicator element
 * @param {string} status - Current connection status
 */
function updateIndicator(indicator, status) {
    const dot = indicator.querySelector('.connection-status-dot');
    const text = indicator.querySelector('.connection-status-text');
    
    // Remove all status classes
    indicator.classList.remove('connected', 'disconnected', 'connecting', 'reconnecting', 'error');
    
    switch (status) {
        case ConnectionStatus.CONNECTED:
            indicator.classList.add('connected');
            text.textContent = 'Connected';
            indicator.title = 'WebSocket connected';
            break;
        case ConnectionStatus.CONNECTING:
            indicator.classList.add('connecting');
            text.textContent = 'Connecting...';
            indicator.title = 'Establishing connection';
            break;
        case ConnectionStatus.RECONNECTING:
            indicator.classList.add('reconnecting');
            text.textContent = 'Reconnecting...';
            indicator.title = 'Attempting to reconnect';
            break;
        case ConnectionStatus.ERROR:
            indicator.classList.add('error');
            text.textContent = 'Error';
            indicator.title = 'Connection error';
            break;
        case ConnectionStatus.DISCONNECTED:
        default:
            indicator.classList.add('disconnected');
            text.textContent = 'Disconnected';
            indicator.title = 'Not connected';
            break;
    }
}

/**
 * Mount the connection status indicator to a container
 * @param {HTMLElement|string} target - Target container element or selector
 * @returns {HTMLElement|null} The indicator element, or null if target not found
 */
export function mountConnectionStatus(target) {
    const container = typeof target === 'string'
        ? document.querySelector(target)
        : target;
    
    if (!container) {
        console.warn('Connection status: target container not found');
        return null;
    }
    
    const indicator = createConnectionStatusIndicator();
    container.appendChild(indicator);
    
    return indicator;
}
