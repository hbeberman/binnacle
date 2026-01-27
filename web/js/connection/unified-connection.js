/**
 * Unified Connection Architecture
 * 
 * Provides a consistent interface for different connection types:
 * - LiveConnection: WebSocket connection to bn gui server
 * - ArchiveConnection: Read-only WASM archive viewer
 * - HostedConnection: Future support for hosted/CDN mode
 * 
 * Usage:
 *   const conn = createConnection({ mode: 'live', wsUrl: 'ws://...' });
 *   await conn.connect();
 *   conn.on('stateChange', () => { ... });
 */

import { ConnectionMode, ConnectionStatus } from '../state.js';
import * as state from '../state.js';
import * as liveConnection from './live-connection.js';
import * as archive from './archive.js';

/**
 * Abstract Connection interface
 * All connection adapters must implement these methods
 */
class Connection {
    constructor() {
        this._listeners = new Map();
        this._status = ConnectionStatus.DISCONNECTED;
    }

    /**
     * Connect to the data source
     * @returns {Promise<void>}
     */
    async connect() {
        throw new Error('connect() must be implemented by subclass');
    }

    /**
     * Disconnect from the data source
     * @returns {Promise<void>}
     */
    async disconnect() {
        throw new Error('disconnect() must be implemented by subclass');
    }

    /**
     * Get current connection status
     * @returns {string} ConnectionStatus value
     */
    getStatus() {
        return this._status;
    }

    /**
     * Check if connection is active
     * @returns {boolean}
     */
    isConnected() {
        return this._status === ConnectionStatus.CONNECTED;
    }

    /**
     * Check if connection is read-only
     * @returns {boolean}
     */
    isReadonly() {
        throw new Error('isReadonly() must be implemented by subclass');
    }

    /**
     * Send data (only for writable connections)
     * @param {Object} _data - Data to send
     * @returns {Promise<boolean>} Success status
     */
    async send(_data) {
        if (this.isReadonly()) {
            console.warn('Cannot send: connection is read-only');
            return false;
        }
        throw new Error('send() must be implemented by subclass');
    }

    /**
     * Request a full state sync (only for live connections)
     * @returns {Promise<void>}
     */
    async requestSync() {
        if (this.isReadonly()) {
            console.warn('Cannot sync: connection is read-only');
            return;
        }
        throw new Error('requestSync() must be implemented by subclass');
    }

    /**
     * Register event listener
     * @param {string} event - Event name (connected, disconnected, stateChange, error)
     * @param {Function} callback - Callback function
     */
    on(event, callback) {
        if (!this._listeners.has(event)) {
            this._listeners.set(event, []);
        }
        this._listeners.get(event).push(callback);
    }

    /**
     * Remove event listener
     * @param {string} event - Event name
     * @param {Function} callback - Callback function
     */
    off(event, callback) {
        if (!this._listeners.has(event)) return;
        const listeners = this._listeners.get(event);
        const index = listeners.indexOf(callback);
        if (index >= 0) {
            listeners.splice(index, 1);
        }
    }

    /**
     * Emit event to all listeners
     * @param {string} event - Event name
     * @param {...any} args - Arguments to pass to listeners
     */
    _emit(event, ...args) {
        if (!this._listeners.has(event)) return;
        for (const callback of this._listeners.get(event)) {
            try {
                callback(...args);
            } catch (error) {
                console.error(`Error in ${event} listener:`, error);
            }
        }
    }

    /**
     * Set connection status and emit status change
     * @param {string} status - ConnectionStatus value
     */
    _setStatus(status) {
        if (this._status !== status) {
            this._status = status;
            state.setConnectionStatus(status);
            this._emit('statusChange', status);
        }
    }
}

/**
 * Live WebSocket Connection Adapter
 * Wraps live-connection.js with unified interface
 */
class LiveConnection extends Connection {
    constructor(config = {}) {
        super();
        this.wsUrl = config.wsUrl;
        this._connected = false;
    }

    async connect() {
        if (this._connected) {
            console.warn('Already connected');
            return;
        }

        this._setStatus(ConnectionStatus.CONNECTING);

        try {
            await liveConnection.connect(this.wsUrl, {
                onConnected: () => {
                    this._connected = true;
                    this._setStatus(ConnectionStatus.CONNECTED);
                    this._emit('connected');
                },
                onDisconnected: () => {
                    this._connected = false;
                    this._setStatus(ConnectionStatus.DISCONNECTED);
                    this._emit('disconnected');
                },
                onError: (error) => {
                    this._setStatus(ConnectionStatus.ERROR);
                    this._emit('error', error);
                },
                onStateChange: (changeType) => {
                    this._emit('stateChange', changeType);
                }
            });
        } catch (error) {
            this._setStatus(ConnectionStatus.ERROR);
            this._emit('error', error);
            throw error;
        }
    }

    async disconnect() {
        if (!this._connected) return;
        liveConnection.disconnect();
        this._connected = false;
        this._setStatus(ConnectionStatus.DISCONNECTED);
        this._emit('disconnected');
    }

    isReadonly() {
        return false;
    }

    async send(data) {
        if (!this._connected) {
            console.warn('Cannot send: not connected');
            return false;
        }
        return liveConnection.send(data);
    }

    async requestSync() {
        if (!this._connected) {
            console.warn('Cannot sync: not connected');
            return;
        }
        return liveConnection.requestSync();
    }
}

/**
 * Archive Connection Adapter
 * Wraps archive.js with unified interface
 * Read-only mode - loads .bng file via WASM
 */
class ArchiveConnection extends Connection {
    constructor(config = {}) {
        super();
        this.archiveUrl = config.archiveUrl;
        this.archiveFile = config.archiveFile; // For file upload
        this._loaded = false;
    }

    async connect() {
        if (this._loaded) {
            console.warn('Archive already loaded');
            return;
        }

        this._setStatus(ConnectionStatus.CONNECTING);

        try {
            let archiveInfo;
            if (this.archiveFile) {
                // Load from File object (drag-drop or file input)
                archiveInfo = await archive.loadArchiveFromFile(this.archiveFile);
            } else if (this.archiveUrl) {
                // Load from URL
                archiveInfo = await archive.loadArchive(this.archiveUrl);
            } else {
                throw new Error('No archive URL or file provided');
            }

            this._loaded = true;
            this._setStatus(ConnectionStatus.CONNECTED);
            
            // Mark state as readonly
            state.set('readonly', true);
            
            this._emit('connected', archiveInfo);
            this._emit('stateChange', 'archive_loaded');
            
            // Run layout if not already positioned
            if (!archive.isLayoutReady()) {
                await archive.runLayout(500, (progress) => {
                    this._emit('layoutProgress', progress);
                });
                this._emit('stateChange', 'layout_complete');
            }
        } catch (error) {
            this._setStatus(ConnectionStatus.ERROR);
            this._emit('error', error);
            throw error;
        }
    }

    async disconnect() {
        if (!this._loaded) return;
        archive.dispose();
        this._loaded = false;
        this._setStatus(ConnectionStatus.DISCONNECTED);
        state.set('readonly', false);
        this._emit('disconnected');
    }

    isReadonly() {
        return true;
    }

    async send(_data) {
        console.warn('Cannot send: archive mode is read-only');
        return false;
    }

    async requestSync() {
        console.warn('Cannot sync: archive mode is read-only');
    }
}

/**
 * Factory function to create the appropriate connection adapter
 * 
 * Auto-detects mode if not specified:
 * - URL param ?ws=HOST:PORT → LiveConnection
 * - URL param ?archive=URL → ArchiveConnection
 * - Explicitly provided config.mode
 * 
 * @param {Object} config - Connection configuration
 * @param {string} [config.mode] - Connection mode (live, archive, auto)
 * @param {string} [config.wsUrl] - WebSocket URL for live mode
 * @param {string} [config.archiveUrl] - Archive URL for archive mode
 * @param {File} [config.archiveFile] - Archive File object for archive mode
 * @returns {Connection} Connection adapter instance
 */
export function createConnection(config = {}) {
    let mode = config.mode;

    // Auto-detect mode if not specified
    if (!mode || mode === 'auto') {
        if (config.wsUrl) {
            mode = 'live';
        } else if (config.archiveUrl || config.archiveFile) {
            mode = 'archive';
        } else {
            throw new Error('Cannot auto-detect mode: no wsUrl, archiveUrl, or archiveFile provided');
        }
    }

    // Normalize mode string
    mode = mode.toLowerCase();

    // Create appropriate adapter
    switch (mode) {
        case 'live':
        case 'websocket':
            if (!config.wsUrl) {
                throw new Error('wsUrl is required for live mode');
            }
            state.setMode(ConnectionMode.WEBSOCKET, { wsUrl: config.wsUrl });
            return new LiveConnection(config);

        case 'archive':
            if (!config.archiveUrl && !config.archiveFile) {
                throw new Error('archiveUrl or archiveFile is required for archive mode');
            }
            state.setMode(ConnectionMode.ARCHIVE, {
                archiveUrl: config.archiveUrl || config.archiveFile?.name
            });
            return new ArchiveConnection(config);

        default:
            throw new Error(`Unknown connection mode: ${mode}`);
    }
}

// Export connection classes for testing and direct instantiation
export { Connection, LiveConnection, ArchiveConnection };

/**
 * Convenience function to create and connect in one call
 * 
 * @param {Object} config - Connection configuration (same as createConnection)
 * @returns {Promise<Connection>} Connected connection instance
 */
export async function connectTo(config) {
    const connection = createConnection(config);
    await connection.connect();
    return connection;
}
