/**
 * Connection Picker Component
 * 
 * Centered card UI for selecting connection method when no URL params are present.
 * Provides:
 * - WebSocket URL input for live mode
 * - File drop zone for local .bng archives
 * - Archive URL input for remote archives
 * - Recent connections list (persisted in localStorage)
 */

import { 
    loadFromStorage, 
    saveToStorage,
    addToast
} from '../state.js';

// Storage key for recent connections
const RECENT_CONNECTIONS_KEY = 'recent_connections';
const MAX_RECENT_CONNECTIONS = 5;

// Connection type enum for recent connections
const ConnectionType = Object.freeze({
    WEBSOCKET: 'websocket',
    ARCHIVE_URL: 'archive_url',
    ARCHIVE_FILE: 'archive_file'
});

/**
 * Get recent connections from localStorage
 * @returns {Array} Array of recent connection objects
 */
export function getRecentConnections() {
    return loadFromStorage(RECENT_CONNECTIONS_KEY, []);
}

/**
 * Add a connection to recent connections list
 * @param {Object} connection - { type, url, name?, timestamp }
 */
export function addRecentConnection(connection) {
    const recent = getRecentConnections();
    
    // Remove existing entry with same URL if present
    const filtered = recent.filter(c => c.url !== connection.url);
    
    // Add new connection at the start
    filtered.unshift({
        ...connection,
        timestamp: Date.now()
    });
    
    // Keep only the most recent N connections
    const trimmed = filtered.slice(0, MAX_RECENT_CONNECTIONS);
    
    saveToStorage(RECENT_CONNECTIONS_KEY, trimmed);
}

/**
 * Remove a connection from recent connections list
 * @param {string} url - The URL to remove
 */
export function removeRecentConnection(url) {
    const recent = getRecentConnections();
    const filtered = recent.filter(c => c.url !== url);
    saveToStorage(RECENT_CONNECTIONS_KEY, filtered);
}

/**
 * Clear all recent connections
 */
export function clearRecentConnections() {
    saveToStorage(RECENT_CONNECTIONS_KEY, []);
}

/**
 * Format a timestamp as a relative time string
 * @param {number} timestamp - Unix timestamp in milliseconds
 * @returns {string} Relative time string (e.g., "2 hours ago")
 */
function formatRelativeTime(timestamp) {
    const now = Date.now();
    const diff = now - timestamp;
    
    const minutes = Math.floor(diff / 60000);
    if (minutes < 1) return 'Just now';
    if (minutes < 60) return `${minutes}m ago`;
    
    const hours = Math.floor(minutes / 60);
    if (hours < 24) return `${hours}h ago`;
    
    const days = Math.floor(hours / 24);
    if (days < 7) return `${days}d ago`;
    
    const weeks = Math.floor(days / 7);
    if (weeks < 4) return `${weeks}w ago`;
    
    return new Date(timestamp).toLocaleDateString();
}

/**
 * Get icon for connection type
 * @param {string} type - Connection type
 * @returns {string} Emoji icon
 */
function getConnectionIcon(type) {
    switch (type) {
        case ConnectionType.WEBSOCKET:
            return '🔌';
        case ConnectionType.ARCHIVE_URL:
        case ConnectionType.ARCHIVE_FILE:
            return '📦';
        default:
            return '🔗';
    }
}

/**
 * Create the connection picker HTML
 * @returns {HTMLElement} The picker overlay element
 */
export function createConnectionPicker() {
    const overlay = document.createElement('div');
    overlay.className = 'connection-picker-overlay';
    overlay.id = 'connection-picker';
    
    overlay.innerHTML = `
        <div class="connection-picker">
            <div class="connection-picker-header">
                <div class="connection-picker-logo">🧭</div>
                <h1 class="connection-picker-title">Binnacle Viewer</h1>
                <p class="connection-picker-subtitle">Connect to a live server or open an archive</p>
            </div>
            
            <div class="connection-options">
                <!-- WebSocket Connection -->
                <div class="connection-option">
                    <div class="connection-option-header">
                        <span class="connection-option-icon">🔌</span>
                        <span class="connection-option-title">Live Connection</span>
                    </div>
                    <p class="connection-option-desc">Connect to a running binnacle GUI server</p>
                    <div class="connection-input-group">
                        <input 
                            type="text" 
                            id="ws-url-input"
                            placeholder="ws://localhost:3030/ws"
                            autocomplete="off"
                            spellcheck="false"
                        >
                        <button class="connection-btn" id="ws-connect-btn">Connect</button>
                    </div>
                </div>
                
                <!-- Archive File -->
                <div class="connection-option">
                    <div class="connection-option-header">
                        <span class="connection-option-icon">📦</span>
                        <span class="connection-option-title">Open Archive</span>
                    </div>
                    <p class="connection-option-desc">Load a .bng archive file (read-only)</p>
                    <div class="file-drop-zone" id="file-drop-zone">
                        <div class="file-drop-zone-icon">📁</div>
                        <p class="file-drop-zone-text">
                            <strong>Drop a .bng file</strong> or click to browse
                        </p>
                        <input type="file" id="file-input" accept=".bng">
                    </div>
                </div>
                
                <!-- Archive URL -->
                <div class="connection-option">
                    <div class="connection-option-header">
                        <span class="connection-option-icon">🌐</span>
                        <span class="connection-option-title">Remote Archive</span>
                    </div>
                    <p class="connection-option-desc">Load an archive from a URL (read-only)</p>
                    <div class="connection-input-group">
                        <input 
                            type="text" 
                            id="archive-url-input"
                            placeholder="https://example.com/project.bng"
                            autocomplete="off"
                            spellcheck="false"
                        >
                        <button class="connection-btn" id="archive-load-btn">Load</button>
                    </div>
                </div>
            </div>
            
            <!-- Recent Connections -->
            <div class="recent-connections" id="recent-connections-section">
                <div class="recent-connections-header">
                    <span class="recent-connections-title">Recent</span>
                    <button class="recent-connections-clear" id="clear-recent-btn">Clear</button>
                </div>
                <div class="recent-connections-list" id="recent-connections-list">
                    <!-- Populated dynamically -->
                </div>
            </div>
            
            <!-- Connection Status -->
            <div class="connection-status hidden" id="connection-status">
                <div class="spinner"></div>
                <span id="connection-status-text">Connecting...</span>
            </div>
        </div>
    `;
    
    return overlay;
}

/**
 * Render the recent connections list
 * @param {HTMLElement} container - The list container element
 */
function renderRecentConnections(container) {
    const section = document.getElementById('recent-connections-section');
    const recent = getRecentConnections();
    
    if (recent.length === 0) {
        section.classList.add('hidden');
        return;
    }
    
    section.classList.remove('hidden');
    
    container.innerHTML = recent.map(conn => `
        <div class="recent-connection-item" data-url="${escapeHtml(conn.url)}" data-type="${conn.type}">
            <span class="recent-connection-icon">${getConnectionIcon(conn.type)}</span>
            <div class="recent-connection-info">
                <div class="recent-connection-url">${escapeHtml(conn.name || conn.url)}</div>
                <div class="recent-connection-time">${formatRelativeTime(conn.timestamp)}</div>
            </div>
            <button class="recent-connection-remove" data-url="${escapeHtml(conn.url)}" title="Remove">×</button>
        </div>
    `).join('');
}

/**
 * Escape HTML to prevent XSS
 * @param {string} str - String to escape
 * @returns {string} Escaped string
 */
function escapeHtml(str) {
    const div = document.createElement('div');
    div.textContent = str;
    return div.innerHTML;
}

/**
 * Show connection status
 * @param {string} text - Status text
 * @param {string} type - 'connecting' or 'error'
 */
function showStatus(text, type = 'connecting') {
    const status = document.getElementById('connection-status');
    const statusText = document.getElementById('connection-status-text');
    
    status.classList.remove('hidden', 'connecting', 'error');
    status.classList.add(type);
    statusText.textContent = text;
    
    // Hide/show spinner based on type
    const spinner = status.querySelector('.spinner');
    if (type === 'error') {
        spinner.classList.add('hidden');
    } else {
        spinner.classList.remove('hidden');
    }
}

/**
 * Hide connection status
 */
function hideStatus() {
    const status = document.getElementById('connection-status');
    status.classList.add('hidden');
}

/**
 * Hide the connection picker
 */
export function hideConnectionPicker() {
    const picker = document.getElementById('connection-picker');
    if (picker) {
        picker.classList.add('hidden');
    }
}

/**
 * Show the connection picker
 */
export function showConnectionPicker() {
    const picker = document.getElementById('connection-picker');
    if (picker) {
        picker.classList.remove('hidden');
        hideStatus();
    }
}

/**
 * Initialize the connection picker with event handlers
 * @param {Object} callbacks - Callback functions { onConnect, onLoadArchive, onLoadFile }
 */
export function initConnectionPicker(callbacks = {}) {
    const picker = document.getElementById('connection-picker');
    if (!picker) {
        console.error('Connection picker not found in DOM');
        return;
    }
    
    // Render recent connections
    const recentList = document.getElementById('recent-connections-list');
    renderRecentConnections(recentList);
    
    // WebSocket connect button
    const wsInput = document.getElementById('ws-url-input');
    const wsConnectBtn = document.getElementById('ws-connect-btn');
    
    const handleWsConnect = () => {
        const url = wsInput.value.trim();
        if (!url) {
            addToast({ type: 'warning', message: 'Please enter a WebSocket URL' });
            wsInput.focus();
            return;
        }
        
        // Validate URL format
        if (!url.startsWith('ws://') && !url.startsWith('wss://')) {
            addToast({ type: 'warning', message: 'URL must start with ws:// or wss://' });
            wsInput.focus();
            return;
        }
        
        showStatus('Connecting to server...');
        wsConnectBtn.disabled = true;
        
        if (callbacks.onConnect) {
            callbacks.onConnect(url)
                .then(() => {
                    addRecentConnection({
                        type: ConnectionType.WEBSOCKET,
                        url: url,
                        name: url.replace(/^wss?:\/\//, '')
                    });
                    hideConnectionPicker();
                })
                .catch(err => {
                    showStatus(err.message || 'Connection failed', 'error');
                    wsConnectBtn.disabled = false;
                });
        }
    };
    
    wsConnectBtn.addEventListener('click', handleWsConnect);
    wsInput.addEventListener('keydown', (e) => {
        if (e.key === 'Enter') handleWsConnect();
    });
    
    // Archive URL load button
    const archiveInput = document.getElementById('archive-url-input');
    const archiveLoadBtn = document.getElementById('archive-load-btn');
    
    const handleArchiveLoad = () => {
        const url = archiveInput.value.trim();
        if (!url) {
            addToast({ type: 'warning', message: 'Please enter an archive URL' });
            archiveInput.focus();
            return;
        }
        
        showStatus('Loading archive...');
        archiveLoadBtn.disabled = true;
        
        if (callbacks.onLoadArchive) {
            callbacks.onLoadArchive(url)
                .then(() => {
                    addRecentConnection({
                        type: ConnectionType.ARCHIVE_URL,
                        url: url,
                        name: url.split('/').pop() || url
                    });
                    hideConnectionPicker();
                })
                .catch(err => {
                    showStatus(err.message || 'Failed to load archive', 'error');
                    archiveLoadBtn.disabled = false;
                });
        }
    };
    
    archiveLoadBtn.addEventListener('click', handleArchiveLoad);
    archiveInput.addEventListener('keydown', (e) => {
        if (e.key === 'Enter') handleArchiveLoad();
    });
    
    // File drop zone
    const dropZone = document.getElementById('file-drop-zone');
    const fileInput = document.getElementById('file-input');
    
    const handleFile = (file) => {
        if (!file) return;
        
        if (!file.name.endsWith('.bng')) {
            addToast({ type: 'warning', message: 'Please select a .bng archive file' });
            return;
        }
        
        showStatus('Loading archive...');
        
        if (callbacks.onLoadFile) {
            callbacks.onLoadFile(file)
                .then(() => {
                    addRecentConnection({
                        type: ConnectionType.ARCHIVE_FILE,
                        url: `file://${file.name}`,
                        name: file.name
                    });
                    hideConnectionPicker();
                })
                .catch(err => {
                    showStatus(err.message || 'Failed to load archive', 'error');
                });
        }
    };
    
    fileInput.addEventListener('change', (e) => {
        handleFile(e.target.files[0]);
    });
    
    // Drag and drop
    dropZone.addEventListener('dragover', (e) => {
        e.preventDefault();
        dropZone.classList.add('drag-over');
    });
    
    dropZone.addEventListener('dragleave', (e) => {
        e.preventDefault();
        dropZone.classList.remove('drag-over');
    });
    
    dropZone.addEventListener('drop', (e) => {
        e.preventDefault();
        dropZone.classList.remove('drag-over');
        
        const files = e.dataTransfer.files;
        if (files.length > 0) {
            handleFile(files[0]);
        }
    });
    
    // Recent connections click handlers
    recentList.addEventListener('click', (e) => {
        // Handle remove button
        const removeBtn = e.target.closest('.recent-connection-remove');
        if (removeBtn) {
            e.stopPropagation();
            const url = removeBtn.dataset.url;
            removeRecentConnection(url);
            renderRecentConnections(recentList);
            return;
        }
        
        // Handle connection item click
        const item = e.target.closest('.recent-connection-item');
        if (item) {
            const url = item.dataset.url;
            const type = item.dataset.type;
            
            if (type === ConnectionType.WEBSOCKET) {
                wsInput.value = url;
                handleWsConnect();
            } else if (type === ConnectionType.ARCHIVE_URL) {
                archiveInput.value = url;
                handleArchiveLoad();
            }
            // Note: archive_file type can't be re-opened directly
        }
    });
    
    // Clear recent connections button
    const clearBtn = document.getElementById('clear-recent-btn');
    clearBtn.addEventListener('click', () => {
        clearRecentConnections();
        renderRecentConnections(recentList);
    });
}

/**
 * Mount the connection picker to the DOM
 * @param {HTMLElement|string} target - Target element or selector
 * @param {Object} callbacks - Callback functions { onConnect, onLoadArchive, onLoadFile }
 */
export function mountConnectionPicker(target, callbacks = {}) {
    const container = typeof target === 'string' 
        ? document.querySelector(target) 
        : target;
    
    if (!container) {
        console.error('Connection picker target not found');
        return;
    }
    
    const picker = createConnectionPicker();
    container.appendChild(picker);
    initConnectionPicker(callbacks);
}
