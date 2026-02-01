/**
 * WebSocket Connection Manager
 * 
 * Manages WebSocket connections to the binnacle GUI server.
 * Handles connection lifecycle, message sending, and event callbacks.
 */

/**
 * Connection states
 */
export const ConnectionState = Object.freeze({
    DISCONNECTED: 'disconnected',
    CONNECTING: 'connecting',
    CONNECTED: 'connected',
    RECONNECTING: 'reconnecting',
    ERROR: 'error'
});

/**
 * WebSocket connection manager
 */
export class WebSocketConnection {
    /**
     * Create a new WebSocket connection manager
     * @param {Object} options - Connection options
     * @param {Function} [options.onOpen] - Called when connection opens (receives connected message)
     * @param {Function} [options.onClose] - Called when connection closes (receives CloseEvent)
     * @param {Function} [options.onError] - Called on connection error (receives Error)
     * @param {Function} [options.onMessage] - Called for each message (receives parsed JSON)
     * @param {boolean} [options.autoReconnect=true] - Automatically reconnect on disconnect
     * @param {number} [options.reconnectInitialDelay=2000] - Initial reconnect delay in ms (default: 2s)
     * @param {number} [options.reconnectMaxDelay=30000] - Max reconnect delay in ms (default: 30s)
     */
    constructor(options = {}) {
        this.options = {
            autoReconnect: true,
            reconnectInitialDelay: 2000,
            reconnectMaxDelay: 30000,
            ...options
        };
        this.ws = null;
        this.state = ConnectionState.DISCONNECTED;
        this.url = null;
        this.serverVersion = null;
        this.reconnectAttempts = 0;
        this.reconnectTimer = null;
        this.manualDisconnect = false;
    }

    /**
     * Connect to WebSocket server
     * @param {string} url - WebSocket URL (e.g., 'ws://localhost:55823/ws')
     * @returns {Promise<void>} Resolves when connection is established
     */
    connect(url) {
        return new Promise((resolve, reject) => {
            if (this.state === ConnectionState.CONNECTED || this.state === ConnectionState.CONNECTING) {
                reject(new Error('Already connected or connecting'));
                return;
            }

            // Check if WebSocket is available
            if (typeof WebSocket === 'undefined') {
                const error = new Error('WebSocket is not supported in this environment');
                this.state = ConnectionState.ERROR;
                this._handleError(error);
                reject(error);
                return;
            }

            this.url = url;
            this.state = ConnectionState.CONNECTING;

            try {
                this.ws = new WebSocket(url);
            } catch (error) {
                this.state = ConnectionState.ERROR;
                this._handleError(error);
                reject(error);
                return;
            }

            // Connection opened
            this.ws.onopen = () => {
                this.state = ConnectionState.CONNECTED;
                this.reconnectAttempts = 0; // Reset on successful connection
                console.log(`WebSocket connected to ${url}`);
            };

            // Handle incoming messages
            this.ws.onmessage = (event) => {
                try {
                    const message = JSON.parse(event.data);
                    
                    // Handle 'connected' message specially
                    if (message.type === 'connected') {
                        this.serverVersion = message.version;
                        console.log(`Server version: ${message.version}`);
                        
                        if (this.options.onOpen) {
                            this.options.onOpen(message);
                        }
                        resolve();
                    } else {
                        // Pass other messages to the handler
                        if (this.options.onMessage) {
                            this.options.onMessage(message);
                        }
                    }
                } catch (error) {
                    console.error('Failed to parse WebSocket message:', error);
                    this._handleError(error);
                }
            };

            // Handle connection close
            this.ws.onclose = (event) => {
                const wasConnected = this.state === ConnectionState.CONNECTED;
                this.state = ConnectionState.DISCONNECTED;
                console.log(`WebSocket disconnected (code: ${event.code}, reason: ${event.reason})`);
                
                if (this.options.onClose) {
                    this.options.onClose(event);
                }

                // If we were resolving the connection and it closed, reject
                if (!wasConnected) {
                    reject(new Error(`Connection closed before established: ${event.reason}`));
                }

                // Auto-reconnect if not manually disconnected
                if (!this.manualDisconnect && this.options.autoReconnect) {
                    this._scheduleReconnect();
                }
            };

            // Handle errors
            this.ws.onerror = (event) => {
                console.error('WebSocket error:', event);
                const error = new Error('WebSocket error');
                this._handleError(error);
                
                // Reject if we're still connecting
                if (this.state === ConnectionState.CONNECTING) {
                    reject(error);
                }
            };
        });
    }

    /**
     * Disconnect from WebSocket server
     */
    disconnect() {
        this.manualDisconnect = true;
        this._cancelReconnect();
        
        if (this.ws) {
            this.ws.close(1000, 'Client disconnect');
            this.ws = null;
        }
        this.state = ConnectionState.DISCONNECTED;
        this.url = null;
        this.serverVersion = null;
        this.reconnectAttempts = 0;
    }

    /**
     * Send a message to the server
     * @param {Object} message - Message object to send (will be JSON stringified)
     * @returns {boolean} True if sent successfully, false otherwise
     */
    send(message) {
        if (this.state !== ConnectionState.CONNECTED || !this.ws) {
            console.warn('Cannot send message: not connected');
            return false;
        }

        try {
            this.ws.send(JSON.stringify(message));
            return true;
        } catch (error) {
            console.error('Failed to send message:', error);
            this._handleError(error);
            return false;
        }
    }

    /**
     * Get current connection state
     * @returns {string} Current state (from ConnectionState enum)
     */
    getState() {
        return this.state;
    }

    /**
     * Check if connected
     * @returns {boolean} True if connected
     */
    isConnected() {
        return this.state === ConnectionState.CONNECTED;
    }

    /**
     * Get server version
     * @returns {number|null} Server version or null if not connected
     */
    getServerVersion() {
        return this.serverVersion;
    }

    /**
     * Get connection URL
     * @returns {string|null} WebSocket URL or null if not connected
     */
    getUrl() {
        return this.url;
    }

    /**
     * Get reconnect attempts count
     * @returns {number} Number of reconnection attempts
     */
    getReconnectAttempts() {
        return this.reconnectAttempts;
    }

    /**
     * Calculate reconnect delay using exponential backoff
     * Starts at 2s, doubles each time, caps at 30s
     * @private
     * @returns {number} Delay in milliseconds
     */
    _calculateReconnectDelay() {
        const delay = this.options.reconnectInitialDelay * Math.pow(2, this.reconnectAttempts);
        return Math.min(delay, this.options.reconnectMaxDelay);
    }

    /**
     * Schedule a reconnection attempt
     * @private
     */
    _scheduleReconnect() {
        // Cancel any existing reconnect timer
        this._cancelReconnect();

        const delay = this._calculateReconnectDelay();
        this.reconnectAttempts++;
        this.state = ConnectionState.RECONNECTING;

        console.log(`Scheduling reconnect attempt #${this.reconnectAttempts} in ${delay}ms`);

        this.reconnectTimer = setTimeout(() => {
            console.log(`Attempting reconnect #${this.reconnectAttempts} to ${this.url}`);
            this.manualDisconnect = false; // Reset flag for reconnect
            this.connect(this.url).catch((error) => {
                console.error('Reconnect failed:', error);
                // _scheduleReconnect will be called again by onclose handler
            });
        }, delay);
    }

    /**
     * Cancel scheduled reconnection
     * @private
     */
    _cancelReconnect() {
        if (this.reconnectTimer) {
            clearTimeout(this.reconnectTimer);
            this.reconnectTimer = null;
        }
    }

    /**
     * Internal error handler
     * @private
     * @param {Error} error - Error object
     */
    _handleError(error) {
        this.state = ConnectionState.ERROR;
        if (this.options.onError) {
            this.options.onError(error);
        }
    }
}

/**
 * Create and connect a WebSocket connection
 * Convenience function for simple use cases
 * @param {string} url - WebSocket URL
 * @param {Object} options - Connection options (same as WebSocketConnection constructor)
 * @returns {Promise<WebSocketConnection>} Connected WebSocket instance
 */
export async function createConnection(url, options = {}) {
    const connection = new WebSocketConnection(options);
    await connection.connect(url);
    return connection;
}
