/**
 * WebSocket Message Handlers
 * 
 * Handles different message types received from the binnacle GUI server.
 * Primary handlers:
 * - 'sync': Full state synchronization (loads all entities/edges at once)
 * - 'reload': Legacy notification to refetch data (backwards compatibility)
 */

import * as state from '../state.js';

/**
 * Message handler callbacks registry
 * @type {Map<string, Function>}
 */
const handlers = new Map();

/**
 * Register a message handler for a specific message type
 * @param {string} type - Message type (e.g., 'sync', 'reload')
 * @param {Function} handler - Handler function(message) => void
 */
export function registerHandler(type, handler) {
    handlers.set(type, handler);
}

/**
 * Handle an incoming WebSocket message
 * Routes to the appropriate handler based on message type.
 * 
 * @param {Object} message - Parsed JSON message from server
 * @returns {boolean} True if message was handled, false otherwise
 */
export function handleMessage(message) {
    if (!message || typeof message.type !== 'string') {
        console.warn('Invalid message format:', message);
        return false;
    }

    const handler = handlers.get(message.type);
    if (handler) {
        try {
            handler(message);
            return true;
        } catch (error) {
            console.error(`Error handling message type "${message.type}":`, error);
            return false;
        }
    }

    // Unknown message type - log but don't fail
    console.debug(`No handler for message type: ${message.type}`);
    return false;
}

/**
 * Callback for fetching full state (used by 'reload' handler)
 * This should be set by the application to point to its data fetching logic.
 * @type {Function|null}
 */
let reloadCallback = null;

/**
 * Set the callback for reload messages
 * The callback should fetch all data from the API and update state.
 * @param {Function} callback - Async function to reload data
 */
export function setReloadCallback(callback) {
    reloadCallback = callback;
}

// ============================================
// Built-in message handlers
// ============================================

/**
 * Handle 'sync' message - full state synchronization
 * 
 * The sync message contains all entities and edges in a single payload.
 * This is used for initial state load and recovery from missed updates.
 * 
 * Expected message format:
 * {
 *   type: 'sync',
 *   version: number,
 *   timestamp: string,
 *   data: {
 *     tasks: [],
 *     bugs: [],
 *     ideas: [],
 *     tests: [],
 *     docs: [],
 *     milestones: [],
 *     queues: [],
 *     agents: [],
 *     edges: [],
 *     ready: [],
 *     log: []  // optional, may be paginated separately
 *   }
 * }
 */
registerHandler('sync', (message) => {
    const { version, timestamp, data } = message;
    
    if (!data) {
        console.warn('sync message missing data payload');
        return;
    }
    
    console.log(`Processing sync message (version: ${version})`);
    
    // Update sync metadata
    state.set('sync.version', version);
    state.set('sync.lastSync', timestamp);
    
    // Populate entities from sync data
    if (data.tasks !== undefined) {
        state.setEntities('tasks', normalizeEntities(data.tasks, 'task'));
    }
    if (data.bugs !== undefined) {
        state.setEntities('bugs', normalizeEntities(data.bugs, 'bug'));
    }
    if (data.ideas !== undefined) {
        state.setEntities('ideas', normalizeEntities(data.ideas, 'idea'));
    }
    if (data.tests !== undefined) {
        state.setEntities('tests', normalizeEntities(data.tests, 'test'));
    }
    if (data.docs !== undefined) {
        state.setEntities('docs', normalizeEntities(data.docs, 'doc'));
    }
    if (data.milestones !== undefined) {
        state.setEntities('milestones', normalizeEntities(data.milestones, 'milestone'));
    }
    if (data.queues !== undefined) {
        state.setEntities('queues', normalizeEntities(data.queues, 'queue'));
    }
    if (data.agents !== undefined) {
        state.setEntities('agents', normalizeEntities(data.agents, 'agent'));
    }
    
    // Set edges
    if (data.edges !== undefined) {
        state.setEdges(normalizeEdges(data.edges));
    }
    
    // Set ready queue
    if (data.ready !== undefined) {
        state.setReady(data.ready);
    }
    
    // Set log entries if provided (may be empty for initial sync)
    if (data.log !== undefined) {
        state.set('log', data.log);
        state.set('logPagination.total', data.log.length);
    }
    
    console.log(`Sync complete: ${countEntities()} entities, ${state.getEdges().length} edges`);
});

/**
 * Handle 'reload' message - legacy notification to refetch all data
 * 
 * For backwards compatibility with the current server which sends
 * 'reload' messages when files change. The client should re-fetch
 * all data from the API endpoints.
 * 
 * Expected message format:
 * {
 *   type: 'reload',
 *   version: number,
 *   timestamp: string
 * }
 */
registerHandler('reload', async (message) => {
    const { version, timestamp } = message;
    
    console.log(`Processing reload message (version: ${version})`);
    
    // Update sync metadata
    state.set('sync.version', version);
    state.set('sync.lastSync', timestamp);
    
    // If a reload callback is registered, call it to fetch fresh data
    if (reloadCallback) {
        try {
            await reloadCallback();
            console.log('Reload complete via callback');
        } catch (error) {
            console.error('Reload callback failed:', error);
        }
    } else {
        console.warn('No reload callback registered - data may be stale');
    }
});

// ============================================
// Helper functions
// ============================================

/**
 * Normalize entities to ensure consistent format
 * @param {Array} entities - Raw entity array
 * @param {string} type - Entity type for default assignment
 * @returns {Array} Normalized entity array
 */
function normalizeEntities(entities, type) {
    if (!Array.isArray(entities)) {
        return [];
    }
    
    return entities.map(entity => ({
        ...entity,
        type: entity.type || type,
        priority: entity.priority ?? getDefaultPriority(type),
        status: entity.status || 'pending',
        tags: entity.tags || [],
        short_name: entity.short_name || null,
        depends_on: entity.depends_on || []
    }));
}

/**
 * Get default priority for entity type
 * @param {string} type - Entity type
 * @returns {number} Default priority (0-4)
 */
function getDefaultPriority(type) {
    switch (type) {
        case 'bug':
            return 2;
        case 'idea':
            return 4;
        case 'queue':
            return 0;
        default:
            return 2;
    }
}

/**
 * Normalize edges to ensure consistent format
 * @param {Array} edges - Raw edge array
 * @returns {Array} Normalized edge array
 */
function normalizeEdges(edges) {
    if (!Array.isArray(edges)) {
        return [];
    }
    
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
 * Count total entities across all types
 * @returns {number} Total entity count
 */
function countEntities() {
    return (
        state.getTasks().length +
        state.getBugs().length +
        state.getIdeas().length +
        state.getTests().length +
        state.getDocs().length +
        state.getMilestones().length +
        state.getAgents().length
    );
}

/**
 * Handle 'entity_added' message - incremental entity addition
 * 
 * Expected message format:
 * {
 *   type: 'entity_added',
 *   entity_type: 'task' | 'bug' | 'idea' | 'test' | 'doc' | 'milestone' | 'queue',
 *   id: string,
 *   entity: object,
 *   version: number,
 *   timestamp: string
 * }
 */
registerHandler('entity_added', (message) => {
    const { entity_type, id, entity, version, timestamp } = message;
    
    if (!entity_type || !id || !entity) {
        console.warn('entity_added message missing required fields:', message);
        return;
    }
    
    console.log(`Adding ${entity_type} ${id} (version: ${version})`);
    
    // Update sync metadata
    state.set('sync.version', version);
    state.set('sync.lastSync', timestamp);
    
    // Map entity_type to state entities key
    const entityKey = getEntityKey(entity_type);
    if (entityKey) {
        const normalized = normalizeEntity(entity, entity_type);
        state.upsertEntity(entityKey, normalized);
    } else {
        console.warn(`Unknown entity type: ${entity_type}`);
    }
});

/**
 * Handle 'entity_updated' message - incremental entity update
 * 
 * Expected message format:
 * {
 *   type: 'entity_updated',
 *   entity_type: string,
 *   id: string,
 *   entity: object,  // Full entity object with all fields
 *   version: number,
 *   timestamp: string
 * }
 */
registerHandler('entity_updated', (message) => {
    const { entity_type, id, entity, version, timestamp } = message;
    
    if (!entity_type || !id || !entity) {
        console.warn('entity_updated message missing required fields:', message);
        return;
    }
    
    console.log(`Updating ${entity_type} ${id} (version: ${version})`);
    
    // Update sync metadata
    state.set('sync.version', version);
    state.set('sync.lastSync', timestamp);
    
    // Map entity_type to state entities key
    const entityKey = getEntityKey(entity_type);
    if (entityKey) {
        const normalized = normalizeEntity(entity, entity_type);
        state.upsertEntity(entityKey, normalized);
    } else {
        console.warn(`Unknown entity type: ${entity_type}`);
    }
});

/**
 * Handle 'entity_removed' message - incremental entity deletion
 * 
 * Expected message format:
 * {
 *   type: 'entity_removed',
 *   entity_type: string,
 *   id: string,
 *   version: number,
 *   timestamp: string
 * }
 */
registerHandler('entity_removed', (message) => {
    const { entity_type, id, version, timestamp } = message;
    
    if (!entity_type || !id) {
        console.warn('entity_removed message missing required fields:', message);
        return;
    }
    
    console.log(`Removing ${entity_type} ${id} (version: ${version})`);
    
    // Update sync metadata
    state.set('sync.version', version);
    state.set('sync.lastSync', timestamp);
    
    // Map entity_type to state entities key
    const entityKey = getEntityKey(entity_type);
    if (entityKey) {
        state.removeEntity(entityKey, id);
    } else {
        console.warn(`Unknown entity type: ${entity_type}`);
    }
});

/**
 * Handle 'edge_added' message - incremental edge addition
 * 
 * Expected message format:
 * {
 *   type: 'edge_added',
 *   id: string,
 *   edge: object,
 *   version: number,
 *   timestamp: string
 * }
 */
registerHandler('edge_added', (message) => {
    const { id, edge, version, timestamp } = message;
    
    if (!id || !edge) {
        console.warn('edge_added message missing required fields:', message);
        return;
    }
    
    console.log(`Adding edge ${id} (version: ${version})`);
    
    // Update sync metadata
    state.set('sync.version', version);
    state.set('sync.lastSync', timestamp);
    
    // Normalize and add edge
    const normalized = normalizeEdge(edge);
    state.addEdge(normalized);
});

/**
 * Handle 'edge_removed' message - incremental edge deletion
 * 
 * Expected message format:
 * {
 *   type: 'edge_removed',
 *   id: string,
 *   edge: object,  // Contains source and target for removal
 *   version: number,
 *   timestamp: string
 * }
 */
registerHandler('edge_removed', (message) => {
    const { id, edge, version, timestamp } = message;
    
    if (!id || !edge) {
        console.warn('edge_removed message missing required fields:', message);
        return;
    }
    
    console.log(`Removing edge ${id} (version: ${version})`);
    
    // Update sync metadata
    state.set('sync.version', version);
    state.set('sync.lastSync', timestamp);
    
    // Remove edge - use ID-based removal
    const edges = state.getEdges().filter(e => e.id !== id);
    state.setEdges(edges);
});

/**
 * Normalize a single edge to ensure consistent format
 * @param {Object} edge - Raw edge object
 * @returns {Object} Normalized edge
 */
function normalizeEdge(edge) {
    return {
        source: edge.source,
        target: edge.target,
        edge_type: edge.edge_type || edge.type || 'related_to',
        id: edge.id || null,
        reason: edge.reason || null,
        weight: edge.weight ?? 1.0,
        created_at: edge.created_at || null
    };
}

/**
 * Map entity_type string from server to state entities key
 * @param {string} entity_type - Server entity type
 * @returns {string|null} State entities key or null if unknown
 */
function getEntityKey(entity_type) {
    const mapping = {
        'task': 'tasks',
        'bug': 'bugs',
        'idea': 'ideas',
        'test': 'tests',
        'doc': 'docs',
        'milestone': 'milestones',
        'queue': 'queues',
        'agent': 'agents'
    };
    
    return mapping[entity_type] || null;
}

/**
 * Normalize a single entity to ensure consistent format
 * @param {Object} entity - Raw entity object
 * @param {string} type - Entity type
 * @returns {Object} Normalized entity
 */
function normalizeEntity(entity, type) {
    return {
        ...entity,
        type: entity.type || type,
        priority: entity.priority ?? getDefaultPriority(type),
        status: entity.status || 'pending',
        tags: entity.tags || [],
        short_name: entity.short_name || null,
        depends_on: entity.depends_on || []
    };
}

/**
 * Get all registered message types
 * @returns {string[]} Array of registered handler types
 */
export function getRegisteredTypes() {
    return Array.from(handlers.keys());
}
