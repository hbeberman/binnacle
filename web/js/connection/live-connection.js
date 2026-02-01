/**
 * Live WebSocket Connection Module
 * 
 * High-level module that integrates WebSocket connection with message handlers.
 * Provides:
 * - Easy connection setup with automatic message routing
 * - Data fetching from REST API for 'reload' messages
 * - State synchronization between server and client
 */

import { WebSocketConnection, ConnectionState } from './websocket.js';
import { handleMessage, setReloadCallback } from './message-handlers.js';
import * as state from '../state.js';
import { ConnectionMode, ConnectionStatus } from '../state.js';

// Module-level connection instance
let connection = null;

// Debounce state for reload messages
let pendingReload = null;
const RELOAD_DEBOUNCE_MS = 100;

/**
 * Connect to a binnacle GUI server
 * 
 * Establishes WebSocket connection and sets up message handlers.
 * Automatically fetches initial data from REST API.
 * 
 * @param {string} wsUrl - WebSocket URL (e.g., 'ws://localhost:3030/ws')
 * @param {Object} options - Connection options
 * @param {Function} [options.onConnected] - Called when connection is established
 * @param {Function} [options.onDisconnected] - Called when connection is lost
 * @param {Function} [options.onError] - Called on connection error
 * @param {Function} [options.onStateChange] - Called when state is updated from server
 * @returns {Promise<WebSocketConnection>} The connection instance
 */
export async function connect(wsUrl, options = {}) {
    // Disconnect existing connection if any
    if (connection) {
        connection.disconnect();
        connection = null;
    }
    
    // Set connection status to connecting
    state.setConnectionStatus(ConnectionStatus.CONNECTING);
    
    // Derive base URL from WebSocket URL
    const baseUrl = deriveBaseUrl(wsUrl);
    
    // Set up reload callback before connecting
    setReloadCallback(() => fetchAllData(baseUrl, options.onStateChange));
    
    // Create connection with message handler
    connection = new WebSocketConnection({
        onOpen: (message) => {
            console.log('Connected to binnacle server');
            state.setConnectionStatus(ConnectionStatus.CONNECTED);
            state.setMode(ConnectionMode.WEBSOCKET, { wsUrl });
            
            // Fetch initial data from REST API
            fetchAllData(baseUrl, options.onStateChange).then(() => {
                if (options.onConnected) {
                    options.onConnected(message);
                }
            }).catch(error => {
                console.error('Failed to fetch initial data:', error);
                if (options.onError) {
                    options.onError(error);
                }
            });
        },
        onClose: (event) => {
            console.log('Disconnected from binnacle server');
            state.setConnectionStatus(ConnectionStatus.DISCONNECTED);
            if (options.onDisconnected) {
                options.onDisconnected(event);
            }
        },
        onError: (error) => {
            // Only log error if it's not a WebSocket unavailability issue
            // (Lightpanda headless browser doesn't support WebSocket)
            if (!error.message || !error.message.includes('WebSocket is not supported')) {
                console.error('WebSocket error:', error);
            }
            state.setConnectionStatus(ConnectionStatus.ERROR);
            if (options.onError) {
                options.onError(error);
            }
        },
        onMessage: (message) => {
            // Route message through handlers
            const handled = routeMessage(message, baseUrl, options.onStateChange);
            
            // Pass unhandled messages to custom handler if provided
            if (!handled && options.onMessage) {
                options.onMessage(message);
            }
        }
    });
    
    await connection.connect(wsUrl);
    return connection;
}

/**
 * Route incoming message to appropriate handler with debouncing
 * @param {Object} message - Parsed message from server
 * @param {string} baseUrl - Base REST API URL
 * @param {Function} onStateChange - Optional callback after state update
 * @returns {boolean} True if message was handled
 */
function routeMessage(message, baseUrl, onStateChange) {
    if (!message || !message.type) {
        return false;
    }
    
    // Debounce reload messages to avoid rapid refetching
    if (message.type === 'reload') {
        if (pendingReload) {
            clearTimeout(pendingReload);
        }
        pendingReload = setTimeout(async () => {
            pendingReload = null;
            handleMessage(message);
            if (onStateChange) {
                onStateChange('reload');
            }
        }, RELOAD_DEBOUNCE_MS);
        return true;
    }
    
    // Handle sync messages immediately (full state update)
    if (message.type === 'sync') {
        handleMessage(message);
        if (onStateChange) {
            onStateChange('sync');
        }
        return true;
    }
    
    // Handle incremental entity messages
    if (message.type === 'entity_added' || message.type === 'entity_updated' || message.type === 'entity_removed') {
        const handled = handleMessage(message);
        if (handled && onStateChange) {
            onStateChange(message.type);
        }
        return handled;
    }
    
    // Handle incremental edge messages
    if (message.type === 'edge_added' || message.type === 'edge_removed') {
        const handled = handleMessage(message);
        if (handled && onStateChange) {
            onStateChange(message.type);
        }
        return handled;
    }
    
    // Try generic message handler
    return handleMessage(message);
}

/**
 * Disconnect from the server
 */
export function disconnect() {
    if (connection) {
        connection.disconnect();
        connection = null;
    }
    
    state.setConnectionStatus(ConnectionStatus.DISCONNECTED);
    
    if (pendingReload) {
        clearTimeout(pendingReload);
        pendingReload = null;
    }
}

/**
 * Get the current connection instance
 * @returns {WebSocketConnection|null}
 */
export function getConnection() {
    return connection;
}

/**
 * Check if currently connected
 * @returns {boolean}
 */
export function isConnected() {
    return connection?.isConnected() ?? false;
}

/**
 * Get current connection state
 * @returns {string} ConnectionState value
 */
export function getConnectionState() {
    return connection?.getState() ?? ConnectionState.DISCONNECTED;
}

/**
 * Send a message to the server
 * @param {Object} message - Message object
 * @returns {boolean} True if sent successfully
 */
export function send(message) {
    if (!connection) {
        console.warn('Cannot send: not connected');
        return false;
    }
    return connection.send(message);
}

/**
 * Request a full state sync from the server
 * Useful when recovering from missed updates.
 * @returns {Promise<void>} Promise that resolves if request sent, rejects otherwise
 */
export function requestSync() {
    return new Promise((resolve, reject) => {
        const lastVersion = state.get('sync.version') || 0;
        const success = send({ type: 'request_sync', last_version: lastVersion });
        if (success) {
            resolve();
        } else {
            reject(new Error('Failed to send sync request: not connected'));
        }
    });
}

// ============================================
// Data fetching helpers
// ============================================

/**
 * Derive base HTTP URL from WebSocket URL
 * @param {string} wsUrl - WebSocket URL (ws:// or wss://)
 * @returns {string} Base HTTP URL
 */
function deriveBaseUrl(wsUrl) {
    return wsUrl
        .replace('wss://', 'https://')
        .replace('ws://', 'http://')
        .replace(/\/ws\/?$/, '');
}

/**
 * Fetch all data from REST API and update state
 * 
 * This is called on initial connection and when 'reload' messages are received.
 * Fetches all entity types, edges, and other data in parallel.
 * 
 * @param {string} baseUrl - Base URL for REST API
 * @param {Function} onStateChange - Optional callback after state update
 */
async function fetchAllData(baseUrl, onStateChange) {
    console.log('Fetching all data from server...');
    
    const logLimit = state.get('logPagination.limit') || 100;
    
    // Fetch all data in parallel
    const [
        tasksData,
        bugsData,
        issuesData,
        ideasData,
        milestonesData,
        readyData,
        testsData,
        docsData,
        logData,
        edgesData,
        agentsData,
        queueData
    ] = await Promise.all([
        fetchJson(`${baseUrl}/api/tasks`),
        fetchJson(`${baseUrl}/api/bugs`),
        fetchJson(`${baseUrl}/api/issues`),
        fetchJson(`${baseUrl}/api/ideas`),
        fetchJson(`${baseUrl}/api/milestones`),
        fetchJson(`${baseUrl}/api/ready`),
        fetchJson(`${baseUrl}/api/tests`),
        fetchJson(`${baseUrl}/api/docs`),
        fetchJson(`${baseUrl}/api/log?limit=${logLimit}&offset=0`),
        fetchJson(`${baseUrl}/api/edges`),
        fetchJson(`${baseUrl}/api/agents`),
        fetchJson(`${baseUrl}/api/queue`)
    ]);
    
    // Normalize and set entities
    state.setEntities('tasks', normalizeItems(tasksData?.tasks, 'task'));
    state.setEntities('bugs', normalizeItems(bugsData?.bugs, 'bug'));
    state.setEntities('issues', normalizeItems(issuesData?.issues, 'issue'));
    state.setEntities('ideas', normalizeItems(ideasData?.ideas, 'idea'));
    state.setEntities('tests', normalizeItems(testsData?.tests, 'test'));
    state.setEntities('docs', normalizeItems(docsData?.docs, 'doc'));
    state.setEntities('milestones', normalizeItems(milestonesData?.milestones, 'milestone'));
    state.setEntities('agents', normalizeAgents(agentsData?.agents));
    
    // Handle queue (single item or null)
    if (queueData?.queue) {
        state.setEntities('queues', [normalizeQueue(queueData.queue)]);
    } else {
        state.setEntities('queues', []);
    }
    
    // Set edges
    state.setEdges(normalizeEdges(edgesData?.edges));
    
    // Set ready items
    state.setReady(readyData?.tasks || []);
    
    // Set log entries with pagination info
    state.set('log', logData?.entries || []);
    state.set('logPagination.total', logData?.total || 0);
    state.set('logPagination.offset', 0);
    state.set('logPagination.hasMore', (logData?.entries?.length || 0) < (logData?.total || 0));
    
    // Update sync timestamp
    state.set('sync.lastSync', new Date().toISOString());
    
    console.log(`Data loaded: ${countEntities()} entities, ${state.getEdges().length} edges`);
    
    if (onStateChange) {
        onStateChange('data_loaded');
    }
}

/**
 * Fetch JSON from URL with error handling
 * @param {string} url - URL to fetch
 * @returns {Promise<Object>} Parsed JSON or empty object on error
 */
async function fetchJson(url) {
    try {
        const response = await fetch(url);
        if (!response.ok) {
            console.warn(`Failed to fetch ${url}: ${response.status}`);
            return {};
        }
        return await response.json();
    } catch (error) {
        console.warn(`Error fetching ${url}:`, error.message);
        return {};
    }
}

/**
 * Normalize items array with type
 * @param {Array} items - Raw items array
 * @param {string} type - Entity type
 * @returns {Array}
 */
function normalizeItems(items, type) {
    if (!Array.isArray(items)) return [];
    return items.map(item => ({
        ...item,
        type: item.type || type,
        priority: item.priority ?? 2,
        status: item.status || 'pending',
        tags: item.tags || [],
        short_name: item.short_name || null,
        depends_on: item.depends_on || []
    }));
}

/**
 * Normalize agents array (filter to worker agents only)
 * @param {Array} agents - Raw agents array
 * @returns {Array}
 */
function normalizeAgents(agents) {
    if (!Array.isArray(agents)) return [];
    // Only include worker agents in the graph
    return agents
        .filter(agent => agent.agent_type === 'worker')
        .map(agent => ({
            id: agent.id || `agent-${agent.pid}`,
            title: agent.name,
            short_name: agent.purpose || agent.name,
            type: 'agent',
            status: agent.status,
            pid: agent.pid,
            container_id: agent.container_id,
            started_at: agent.started_at,
            last_heartbeat: agent.last_heartbeat,
            // Keep original agent data for renderer (used by drawAgentLabel)
            _agent: agent
        }));
}

/**
 * Normalize queue to entity format
 * @param {Object} queue - Raw queue object
 * @returns {Object}
 */
function normalizeQueue(queue) {
    return {
        ...queue,
        type: 'queue',
        status: 'active',
        priority: 0,
        short_name: queue.title || null,
        depends_on: []
    };
}

/**
 * Normalize edges array
 * @param {Array} edges - Raw edges array
 * @returns {Array}
 */
function normalizeEdges(edges) {
    if (!Array.isArray(edges)) return [];
    return edges.map(edge => ({
        source: edge.source,
        target: edge.target,
        edge_type: edge.edge_type || edge.type || 'related_to',
        id: edge.id || null,
        reason: edge.reason || null,
        weight: edge.weight ?? 1.0,
        created_at: edge.created_at || null
    }));
}

/**
 * Count total entities
 * @returns {number}
 */
function countEntities() {
    return (
        state.getTasks().length +
        state.getBugs().length +
        state.getIssues().length +
        state.getIdeas().length +
        state.getTests().length +
        state.getDocs().length +
        state.getMilestones().length +
        state.getAgents().length +
        (state.get('entities.queues')?.length || 0)
    );
}
