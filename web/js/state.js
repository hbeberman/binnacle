/**
 * Binnacle Viewer State Management
 * 
 * Central state store for the decoupled viewer. Provides:
 * - Global state object with all viewer data
 * - Getter/setter functions for safe state access
 * - Event system for state change notifications
 */

// Connection modes
export const ConnectionMode = Object.freeze({
    NONE: 'none',           // No connection yet (picker shown)
    WEBSOCKET: 'websocket', // Live WebSocket to bn gui server
    ARCHIVE: 'archive'      // Static .bng archive file
});

// Connection states (matches ConnectionState from websocket.js)
export const ConnectionStatus = Object.freeze({
    DISCONNECTED: 'disconnected',
    CONNECTING: 'connecting',
    CONNECTED: 'connected',
    RECONNECTING: 'reconnecting',
    ERROR: 'error'
});

// Default state structure
const createDefaultState = () => ({
    // Connection state
    mode: ConnectionMode.NONE,
    connectionStatus: ConnectionStatus.DISCONNECTED,
    readonly: false,
    wsUrl: null,
    archiveUrl: null,
    
    // Entity data (from server or archive)
    entities: {
        tasks: [],
        bugs: [],
        ideas: [],
        tests: [],
        docs: [],
        milestones: [],
        queues: [],
        agents: []
    },
    
    // Edge/link data
    edges: [],
    
    // Ready queue cache
    ready: [],
    
    // Log entries
    log: [],
    logPagination: {
        total: 0,
        offset: 0,
        limit: 100,
        loading: false,
        hasMore: true
    },
    
    // Commits for timeline
    commits: [],
    
    // UI state
    ui: {
        currentView: 'graph',  // 'graph', 'nodes', 'agents', 'log'
        sidebarCollapsed: false,
        
        // Graph viewport
        viewport: {
            panX: 0,
            panY: 0,
            zoom: 1.0,
            minZoom: 0.1,
            maxZoom: 3.0
        },
        
        // Selection state
        selectedNode: null,
        selectedEdge: null,
        hoveredNode: null,
        hoveredEdge: null,
        
        // Auto-follow configuration
        autoFollow: true,
        followTargetId: 'auto',
        followingNodeId: null,
        userPaused: false,  // Set to true when user manually pans/zooms
        autoFollowConfig: {
            nodeTypes: { task: true, bug: true, idea: false, test: false, doc: false },
            focusDelaySeconds: 10
        },
        
        // Filters
        nodeTypeFilters: {
            task: true,
            bug: true,
            idea: true,
            test: true,
            doc: true,
            milestone: true,
            queue: true,
            agent: true
        },
        edgeTypeFilters: {
            depends_on: true,
            blocks: true,
            related_to: true,
            child_of: true,
            parent_of: true,
            tests: true,
            fixes: true,
            queued: true,
            working_on: true,
            documents: true
        },
        hideCompleted: true,
        searchQuery: '',
        
        // Graph physics (for live mode)
        physics: {
            damping: 0.88,
            gravityStrength: 0.01,
            repulsionStrength: 15000,
            springStrength: 0.1,
            springRestingLength: 200
        },
        
        // Info panel state
        infoPanelOpen: false,
        infoPanelTab: 'details',  // 'details', 'activity', 'commits'
        
        // Toast notifications
        toasts: []
    },
    
    // Sync state (for incremental updates)
    sync: {
        version: 0,
        lastSync: null,
        pendingChanges: []
    }
});

// The global state instance
let state = createDefaultState();

// Event listeners for state changes
const listeners = new Map();

/**
 * Subscribe to state changes
 * @param {string} path - Dot-notation path to watch (e.g., 'ui.currentView', 'entities.tasks')
 *                        Use '*' to subscribe to all changes
 *                        Use 'prefix.*' to subscribe to all changes under a prefix (e.g., 'entities.*')
 * @param {Function} callback - Called with (newValue, oldValue, path)
 * @returns {Function} Unsubscribe function
 */
export function subscribe(path, callback) {
    if (!listeners.has(path)) {
        listeners.set(path, new Set());
    }
    listeners.get(path).add(callback);
    
    return () => {
        const pathListeners = listeners.get(path);
        if (pathListeners) {
            pathListeners.delete(callback);
            if (pathListeners.size === 0) {
                listeners.delete(path);
            }
        }
    };
}

/**
 * Notify listeners of a state change
 * @param {string} path - The path that changed
 * @param {*} newValue - New value
 * @param {*} oldValue - Previous value
 */
function notifyListeners(path, newValue, oldValue) {
    // Notify specific path listeners
    const pathListeners = listeners.get(path);
    if (pathListeners) {
        for (const callback of pathListeners) {
            try {
                callback(newValue, oldValue, path);
            } catch (e) {
                console.error(`State listener error for path "${path}":`, e);
            }
        }
    }
    
    // Notify prefix pattern listeners (e.g., 'entities.*' matches 'entities.tasks')
    for (const [pattern, patternListeners] of listeners) {
        if (pattern.endsWith('.*')) {
            const prefix = pattern.slice(0, -2); // Remove '.*' suffix
            if (path.startsWith(prefix + '.')) {
                for (const callback of patternListeners) {
                    try {
                        callback(newValue, oldValue, path);
                    } catch (e) {
                        console.error(`State listener error for pattern "${pattern}":`, e);
                    }
                }
            }
        }
    }
    
    // Notify wildcard listeners
    const wildcardListeners = listeners.get('*');
    if (wildcardListeners) {
        for (const callback of wildcardListeners) {
            try {
                callback(newValue, oldValue, path);
            } catch (e) {
                console.error('State wildcard listener error:', e);
            }
        }
    }
}

/**
 * Get a value from state by dot-notation path
 * @param {string} path - Dot-notation path (e.g., 'ui.currentView', 'entities.tasks')
 * @returns {*} The value at the path, or undefined if not found
 */
export function get(path) {
    const parts = path.split('.');
    let current = state;
    
    for (const part of parts) {
        if (current === null || current === undefined) {
            return undefined;
        }
        current = current[part];
    }
    
    return current;
}

/**
 * Set a value in state by dot-notation path
 * @param {string} path - Dot-notation path (e.g., 'ui.currentView')
 * @param {*} value - The value to set
 */
export function set(path, value) {
    const parts = path.split('.');
    const lastPart = parts.pop();
    let current = state;
    
    // Navigate to the parent object
    for (const part of parts) {
        if (current[part] === undefined) {
            current[part] = {};
        }
        current = current[part];
    }
    
    const oldValue = current[lastPart];
    current[lastPart] = value;
    
    notifyListeners(path, value, oldValue);
}

/**
 * Update multiple values in state at once (batch update)
 * @param {Object} updates - Object with path: value pairs
 */
export function batch(updates) {
    for (const [path, value] of Object.entries(updates)) {
        set(path, value);
    }
}

/**
 * Get the entire state object (read-only access)
 * @returns {Object} The current state (shallow copy)
 */
export function getState() {
    return { ...state };
}

/**
 * Reset state to defaults
 */
export function reset() {
    const oldState = state;
    state = createDefaultState();
    notifyListeners('*', state, oldState);
}

// ============================================
// Convenience getters for common state paths
// ============================================

export function getMode() {
    return state.mode;
}

export function isReadonly() {
    return state.readonly;
}

export function getTasks() {
    return state.entities.tasks;
}

export function getBugs() {
    return state.entities.bugs;
}

export function getIdeas() {
    return state.entities.ideas;
}

export function getTests() {
    return state.entities.tests;
}

export function getDocs() {
    return state.entities.docs;
}

export function getMilestones() {
    return state.entities.milestones;
}

export function getAgents() {
    return state.entities.agents;
}

export function getEdges() {
    return state.edges;
}

export function getReady() {
    return state.ready;
}

/**
 * Get a node by ID from any entity type
 * @param {string} id - Node ID
 * @returns {Object|null} Node object or null if not found
 */
export function getNode(id) {
    // Search in all entity types
    const entityTypes = ['tasks', 'bugs', 'ideas', 'tests', 'docs', 'milestones', 'queues', 'agents'];
    
    for (const type of entityTypes) {
        const entities = state.entities[type];
        const node = entities.find(e => e.id === id);
        if (node) {
            return node;
        }
    }
    
    return null;
}

export function getCurrentView() {
    return state.ui.currentView;
}

export function getViewport() {
    return state.ui.viewport;
}

export function getSelectedNode() {
    return state.ui.selectedNode;
}

export function getSelectedEdge() {
    return state.ui.selectedEdge;
}

// ============================================
// Convenience setters for common operations
// ============================================

export function setMode(mode, options = {}) {
    set('mode', mode);
    if (options.wsUrl !== undefined) {
        set('wsUrl', options.wsUrl);
    }
    if (options.archiveUrl !== undefined) {
        set('archiveUrl', options.archiveUrl);
    }
    set('readonly', mode === ConnectionMode.ARCHIVE);
}

export function setCurrentView(view) {
    set('ui.currentView', view);
}

export function setSelectedNode(nodeId) {
    set('ui.selectedNode', nodeId);
}

export function setSelectedEdge(edgeId) {
    set('ui.selectedEdge', edgeId);
}

export function setViewport(viewport) {
    set('ui.viewport', { ...state.ui.viewport, ...viewport });
}

export function getConnectionStatus() {
    return state.connectionStatus;
}

export function setConnectionStatus(status) {
    set('connectionStatus', status);
}

// ============================================
// Entity update helpers
// ============================================

/**
 * Replace all entities of a given type
 * @param {string} type - Entity type (tasks, bugs, ideas, etc.)
 * @param {Array} entities - New entity array
 */
export function setEntities(type, entities) {
    set(`entities.${type}`, entities);
}

/**
 * Add or update a single entity
 * @param {string} type - Entity type
 * @param {Object} entity - Entity to add/update (must have 'id' field)
 */
export function upsertEntity(type, entity) {
    const entities = [...state.entities[type]];
    const index = entities.findIndex(e => e.id === entity.id);
    
    if (index >= 0) {
        entities[index] = entity;
    } else {
        entities.push(entity);
    }
    
    set(`entities.${type}`, entities);
}

/**
 * Remove an entity by ID
 * @param {string} type - Entity type
 * @param {string} id - Entity ID to remove
 */
export function removeEntity(type, id) {
    const entities = state.entities[type].filter(e => e.id !== id);
    set(`entities.${type}`, entities);
}

/**
 * Set all edges
 * @param {Array} edges - New edges array
 */
export function setEdges(edges) {
    set('edges', edges);
}

/**
 * Add an edge
 * @param {Object} edge - Edge to add
 */
export function addEdge(edge) {
    const edges = [...state.edges, edge];
    set('edges', edges);
}

/**
 * Remove an edge
 * @param {string} sourceId - Source entity ID
 * @param {string} targetId - Target entity ID
 */
export function removeEdge(sourceId, targetId) {
    const edges = state.edges.filter(
        e => !(e.source === sourceId && e.target === targetId)
    );
    set('edges', edges);
}

/**
 * Set ready queue
 * @param {Array} ready - Ready items
 */
export function setReady(ready) {
    set('ready', ready);
}

// ============================================
// Toast notification helpers
// ============================================

let toastIdCounter = 0;

/**
 * Add a toast notification
 * @param {Object} toast - Toast config { type: 'info'|'success'|'warning'|'error', message: string, duration?: number }
 * @returns {number} Toast ID for manual dismissal
 */
export function addToast(toast) {
    const id = ++toastIdCounter;
    const toasts = [...state.ui.toasts, { ...toast, id }];
    set('ui.toasts', toasts);
    
    // Auto-dismiss after duration (default 5 seconds)
    const duration = toast.duration ?? 5000;
    if (duration > 0) {
        setTimeout(() => dismissToast(id), duration);
    }
    
    return id;
}

/**
 * Dismiss a toast by ID
 * @param {number} id - Toast ID
 */
export function dismissToast(id) {
    const toasts = state.ui.toasts.filter(t => t.id !== id);
    set('ui.toasts', toasts);
}

// ============================================
// LocalStorage persistence helpers
// ============================================

const STORAGE_KEY_PREFIX = 'binnacle_viewer_';

/**
 * Load a value from localStorage
 * @param {string} key - Storage key (without prefix)
 * @param {*} defaultValue - Default if not found
 * @returns {*} Parsed value or default
 */
export function loadFromStorage(key, defaultValue) {
    try {
        const stored = localStorage.getItem(STORAGE_KEY_PREFIX + key);
        if (stored !== null) {
            return JSON.parse(stored);
        }
    } catch (e) {
        console.warn(`Failed to load ${key} from localStorage:`, e);
    }
    return defaultValue;
}

/**
 * Save a value to localStorage
 * @param {string} key - Storage key (without prefix)
 * @param {*} value - Value to store
 */
export function saveToStorage(key, value) {
    try {
        localStorage.setItem(STORAGE_KEY_PREFIX + key, JSON.stringify(value));
    } catch (e) {
        console.warn(`Failed to save ${key} to localStorage:`, e);
    }
}

/**
 * Initialize UI state from localStorage
 * Call this after state module loads to restore user preferences
 */
export function initFromStorage() {
    const nodeFilters = loadFromStorage('nodeTypeFilters', null);
    if (nodeFilters) {
        set('ui.nodeTypeFilters', nodeFilters);
    }
    
    const edgeFilters = loadFromStorage('edgeTypeFilters', null);
    if (edgeFilters) {
        set('ui.edgeTypeFilters', edgeFilters);
    }
    
    const autoFollowConfig = loadFromStorage('autoFollowConfig', null);
    if (autoFollowConfig) {
        set('ui.autoFollowConfig', autoFollowConfig);
    }
    
    const hideCompleted = loadFromStorage('hideCompleted', null);
    if (hideCompleted !== null) {
        set('ui.hideCompleted', hideCompleted);
    }
}

// Auto-persist certain UI settings when they change
subscribe('ui.nodeTypeFilters', (value) => saveToStorage('nodeTypeFilters', value));
subscribe('ui.edgeTypeFilters', (value) => saveToStorage('edgeTypeFilters', value));
subscribe('ui.autoFollowConfig', (value) => saveToStorage('autoFollowConfig', value));
subscribe('ui.hideCompleted', (value) => saveToStorage('hideCompleted', value));
