/**
 * Connection Status Indicator Component
 * 
 * Displays connection status in the header with mode-aware badges:
 * - 🟢 Connected (Live WebSocket)
 * - 🔴 Disconnected
 * - 📦 Archive (Read-only)
 * - ⏳ Loading/Connecting
 * 
 * Updates automatically on connection state changes.
 */

import { 
    subscribe, 
    getConnectionStatus,
    getMode,
    ConnectionStatus,
    ConnectionMode
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
        updateIndicator(indicator, status, getMode());
    });
    
    // Subscribe to mode changes
    subscribe('mode', (mode) => {
        updateIndicator(indicator, getConnectionStatus(), mode);
    });
    
    // Initialize with current state
    updateIndicator(indicator, getConnectionStatus(), getMode());
    
    return indicator;
}

/**
 * Update the indicator display based on connection status and mode
 * @param {HTMLElement} indicator - The indicator element
 * @param {string} status - Current connection status
 * @param {string} mode - Current connection mode
 */
function updateIndicator(indicator, status, mode) {
    const dot = indicator.querySelector('.connection-status-dot');
    const text = indicator.querySelector('.connection-status-text');
    
    // Remove all status classes
    indicator.classList.remove('connected', 'disconnected', 'connecting', 'reconnecting', 'error', 'archive');
    
    // Archive mode gets special treatment
    if (mode === ConnectionMode.ARCHIVE) {
        if (status === ConnectionStatus.CONNECTED) {
            indicator.classList.add('archive');
            dot.textContent = '📦';
            text.textContent = 'Archive';
            indicator.title = 'Archive mode (read-only)';
        } else if (status === ConnectionStatus.CONNECTING) {
            indicator.classList.add('connecting');
            dot.textContent = '⏳';
            text.textContent = 'Loading...';
            indicator.title = 'Loading archive';
        } else {
            indicator.classList.add('error');
            dot.textContent = '🔴';
            text.textContent = 'Error';
            indicator.title = 'Failed to load archive';
        }
        return;
    }
    
    // Live/WebSocket mode
    switch (status) {
        case ConnectionStatus.CONNECTED:
            indicator.classList.add('connected');
            dot.textContent = '🟢';
            text.textContent = 'Connected';
            indicator.title = 'WebSocket connected';
            break;
        case ConnectionStatus.CONNECTING:
            indicator.classList.add('connecting');
            dot.textContent = '⏳';
            text.textContent = 'Connecting...';
            indicator.title = 'Establishing connection';
            break;
        case ConnectionStatus.RECONNECTING:
            indicator.classList.add('reconnecting');
            dot.textContent = '⏳';
            text.textContent = 'Reconnecting...';
            indicator.title = 'Attempting to reconnect';
            break;
        case ConnectionStatus.ERROR:
            indicator.classList.add('error');
            dot.textContent = '🔴';
            text.textContent = 'Error';
            indicator.title = 'Connection error';
            break;
        case ConnectionStatus.DISCONNECTED:
        default:
            indicator.classList.add('disconnected');
            dot.textContent = '🔴';
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
