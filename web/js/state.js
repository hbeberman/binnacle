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
        selectedNode: null,        // For backward compatibility (single selection)
        selectedNodes: [],         // Multi-selection array
        selectedEdge: null,
        hoveredNode: null,
        hoveredEdge: null,
        focusedNode: null,  // Node to keep centered until dismissed
        
        // Auto-follow configuration
        autoFollow: true,
        followTargetId: 'auto',
        followingNodeId: null,
        pinnedAgentId: null,  // ID of manually pinned agent (takes priority over auto-follow)
        agentGoodbyeActive: null,  // ID of agent in goodbye state (if any)
        goodbyeStartTime: null,  // Timestamp when goodbye state started (for 3s linger)
        userPaused: false,  // Set to true when user manually pans/zooms
        followTypeFilter: 'agent',  // Follow mode type filter ('none', '', 'task', 'bug', 'idea', 'agent')
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
        edgePhysicsFilters: {
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
            gravityStrength: 0.002,  // Reduced from 0.01 to allow nodes to float more naturally
            repulsionStrength: 25000,
            springStrength: 0.1,
            springRestingLength: 300,
            // Edge-type-specific resting lengths (overrides default)
            edgeRestingLengths: {
                working_on: 150  // 50% of default (300) for agent-to-task edges
            }
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
    let notifiedCount = 0;
    
    // Notify specific path listeners
    const pathListeners = listeners.get(path);
    if (pathListeners) {
        for (const callback of pathListeners) {
            try {
                callback(newValue, oldValue, path);
                notifiedCount++;
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
                        notifiedCount++;
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
                notifiedCount++;
            } catch (e) {
                console.error('State wildcard listener error:', e);
            }
        }
    }
    
    // Debug log for entity changes
    if (path.startsWith('entities.') && Array.isArray(newValue)) {
        console.log(`[State.notify] "${path}" notified ${notifiedCount} listeners`);
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
            console.warn(`[State.get] Path "${path}" failed at "${part}": current is ${current}`);
            return undefined;
        }
        current = current[part];
    }
    
    // Debug logging for entity queries
    if (path.startsWith('entities.') && Array.isArray(current)) {
        console.log(`[State.get] "${path}" => array with ${current.length} items`);
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
    
    // Debug logging for entity updates
    if (path.startsWith('entities.') && Array.isArray(value)) {
        console.log(`[State.set] "${path}" => array with ${value.length} items`);
    }
    
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

/**
 * Get all entities as a flat array
 * @returns {Array} Combined array of all entities
 */
export function getEntities() {
    return [
        ...state.entities.tasks,
        ...state.entities.bugs,
        ...state.entities.ideas,
        ...state.entities.tests,
        ...state.entities.docs,
        ...state.entities.milestones,
        ...state.entities.queues,
        ...state.entities.agents
    ];
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
    // Also update selectedNodes array for consistency
    set('ui.selectedNodes', nodeId ? [nodeId] : []);
}

/**
 * View a node on the graph: switch to graph view, pan to node, and select it
 * @param {string} nodeId - Node ID to view
 * @param {Object} options - Optional configuration
 * @param {number} options.duration - Pan animation duration in ms (default: 500)
 * @param {number} options.targetZoom - Target zoom level (default: 1.5)
 */
export function viewNodeOnGraph(nodeId, options = {}) {
    const { duration = 500, targetZoom = 1.5 } = options;
    
    // Get the node to find its position
    const node = getNode(nodeId);
    if (!node) {
        console.warn(`Entity ${nodeId} not found`);
        return;
    }
    
    // Switch to graph view
    setCurrentView('graph');
    
    // Dynamically import panToNode to avoid circular dependencies
    // panToNode is in graph/index.js which may import state.js
    import('./graph/index.js').then(({ panToNode }) => {
        // Pan to the node's position if coordinates exist
        if (typeof node.x === 'number' && typeof node.y === 'number') {
            panToNode(node.x, node.y, { duration, targetZoom });
        }
    });
    
    // Select the node
    setSelectedNode(nodeId);
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
// Multi-selection helpers
// ============================================

/**
 * Get all selected node IDs
 * @returns {Array<string>} Array of selected node IDs
 */
export function getSelectedNodes() {
    return state.ui.selectedNodes;
}

/**
 * Check if a node is currently selected
 * @param {string} nodeId - Node ID to check
 * @returns {boolean} True if node is selected
 */
export function isSelected(nodeId) {
    return state.ui.selectedNodes.includes(nodeId);
}

/**
 * Toggle selection of a node
 * @param {string} nodeId - Node ID to toggle
 * @param {boolean} clearOthers - If true, clear other selections first (default: false)
 */
export function toggleSelection(nodeId, clearOthers = false) {
    let selectedNodes;
    
    if (clearOthers) {
        // Single select mode: clear others first
        selectedNodes = [nodeId];
    } else {
        // Multi-select mode: toggle this node
        if (state.ui.selectedNodes.includes(nodeId)) {
            selectedNodes = state.ui.selectedNodes.filter(id => id !== nodeId);
        } else {
            selectedNodes = [...state.ui.selectedNodes, nodeId];
        }
    }
    
    set('ui.selectedNodes', selectedNodes);
    
    // Update single selectedNode for backward compatibility
    // Use the last selected node, or null if none selected
    set('ui.selectedNode', selectedNodes.length > 0 ? selectedNodes[selectedNodes.length - 1] : null);
}

/**
 * Set multiple nodes as selected (replaces current selection)
 * @param {Array<string>} nodeIds - Array of node IDs to select
 */
export function setSelectedNodes(nodeIds) {
    set('ui.selectedNodes', [...nodeIds]);
    
    // Update single selectedNode for backward compatibility
    set('ui.selectedNode', nodeIds.length > 0 ? nodeIds[nodeIds.length - 1] : null);
}

/**
 * Clear all node selections
 */
export function clearSelection() {
    set('ui.selectedNodes', []);
    set('ui.selectedNode', null);
}

/**
 * Select all visible nodes (respecting current filters)
 * @param {Array<Object>} visibleNodes - Array of visible node objects
 */
export function selectAll(visibleNodes) {
    const nodeIds = visibleNodes.map(node => node.id);
    setSelectedNodes(nodeIds);
}

// ============================================
// Selection persistence helpers
// ============================================

/**
 * Persist current selection to localStorage
 */
export function persistSelection() {
    saveToStorage('selectedNodes', state.ui.selectedNodes);
}

/**
 * Restore selection from localStorage
 */
export function restoreSelection() {
    const storedSelection = loadFromStorage('selectedNodes', []);
    if (storedSelection && storedSelection.length > 0) {
        setSelectedNodes(storedSelection);
    }
}

/**
 * Clear persisted selection from localStorage
 */
export function clearPersistedSelection() {
    saveToStorage('selectedNodes', []);
}

// ============================================
// Undo/Redo support for batch operations
// ============================================

// Undo stack (stores operation records)
const undoStack = [];
const MAX_UNDO_STACK_SIZE = 50;

/**
 * Record a batch operation for undo support
 * @param {Object} operation - Operation record
 * @param {string} operation.type - Operation type ('batch-close', 'batch-queue-add', etc.)
 * @param {Array<Object>} operation.changes - Array of changes (entity snapshots before/after)
 * @param {string} operation.description - Human-readable description
 * @param {Function} operation.undo - Undo function
 */
export function recordBatchOperation(operation) {
    undoStack.push({
        ...operation,
        timestamp: Date.now()
    });
    
    // Limit stack size
    if (undoStack.length > MAX_UNDO_STACK_SIZE) {
        undoStack.shift();
    }
    
    notifyListeners('undo.stack', undoStack);
}

/**
 * Undo the last batch operation
 * @returns {Object|null} The undone operation, or null if stack is empty
 */
export function undoLastOperation() {
    if (undoStack.length === 0) {
        return null;
    }
    
    const operation = undoStack.pop();
    
    try {
        if (operation.undo && typeof operation.undo === 'function') {
            operation.undo();
        }
        notifyListeners('undo.stack', undoStack);
        return operation;
    } catch (e) {
        console.error('Failed to undo operation:', e);
        // Re-add operation to stack if undo failed
        undoStack.push(operation);
        throw e;
    }
}

/**
 * Get the undo stack
 * @returns {Array} Array of operation records
 */
export function getUndoStack() {
    return [...undoStack];
}

/**
 * Check if undo is available
 * @returns {boolean} True if there are operations to undo
 */
export function canUndo() {
    return undoStack.length > 0;
}

/**
 * Clear the undo stack
 */
export function clearUndoStack() {
    undoStack.length = 0;
    notifyListeners('undo.stack', undoStack);
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
    
    const edgePhysicsFilters = loadFromStorage('edgePhysicsFilters', null);
    if (edgePhysicsFilters) {
        set('ui.edgePhysicsFilters', edgePhysicsFilters);
    }
    
    const autoFollow = loadFromStorage('autoFollow', null);
    if (autoFollow !== null) {
        set('ui.autoFollow', autoFollow);
    }
    
    const autoFollowConfig = loadFromStorage('autoFollowConfig', null);
    if (autoFollowConfig) {
        set('ui.autoFollowConfig', autoFollowConfig);
    }
    
    const hideCompleted = loadFromStorage('hideCompleted', null);
    if (hideCompleted !== null) {
        set('ui.hideCompleted', hideCompleted);
    }
    
    const followTypeFilter = loadFromStorage('followTypeFilter', null);
    if (followTypeFilter !== null) {
        set('ui.followTypeFilter', followTypeFilter);
    }
    
    // Restore selection (but only if not empty - empty means user cleared it)
    const selectedNodes = loadFromStorage('selectedNodes', null);
    if (selectedNodes && selectedNodes.length > 0) {
        set('ui.selectedNodes', selectedNodes);
        set('ui.selectedNode', selectedNodes[selectedNodes.length - 1]);
    }
}

// Auto-persist certain UI settings when they change
subscribe('ui.nodeTypeFilters', (value) => saveToStorage('nodeTypeFilters', value));
subscribe('ui.edgeTypeFilters', (value) => saveToStorage('edgeTypeFilters', value));
subscribe('ui.edgePhysicsFilters', (value) => saveToStorage('edgePhysicsFilters', value));
subscribe('ui.autoFollow', (value) => saveToStorage('autoFollow', value));
subscribe('ui.autoFollowConfig', (value) => saveToStorage('autoFollowConfig', value));
subscribe('ui.hideCompleted', (value) => saveToStorage('hideCompleted', value));
subscribe('ui.followTypeFilter', (value) => saveToStorage('followTypeFilter', value));

// Persist selection when changed (debounced to avoid excessive writes)
let selectionPersistTimer = null;
subscribe('ui.selectedNodes', (value) => {
    if (selectionPersistTimer) {
        clearTimeout(selectionPersistTimer);
    }
    selectionPersistTimer = setTimeout(() => {
        saveToStorage('selectedNodes', value);
    }, 500); // Debounce for 500ms
});


/**
 * Debug function: get the internal state object reference
 * Used to verify that all modules are using the same state instance
 */
export function _getStateObjectRef() {
    return state;
}
