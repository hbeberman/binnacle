/**
 * WebSocket Message Handlers
 * 
 * Handles different message types received from the binnacle GUI server.
 * Primary handlers:
 * - 'sync': Full state synchronization (loads all entities/edges at once)
 * - 'reload': Legacy notification to refetch data (backwards compatibility)
 */

import * as state from '../state.js';
import { checkVersionConflict } from './version-conflict.js';

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
 * Handle 'sync' or 'state' message - full state synchronization
 * 
 * The sync/state message contains all entities and edges in a single payload.
 * This is used for initial state load and recovery from missed updates.
 * 
 * Note: The server sends 'state' type messages with 'links' field, while
 * legacy clients may send 'sync' type with 'edges' field. Both are supported.
 * 
 * Expected message format:
 * {
 *   type: 'sync' | 'state',
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
 *     edges: [] | links: [],  // server sends 'links', legacy may send 'edges'
 *     ready: [],
 *     log: []  // optional, may be paginated separately
 *   }
 * }
 */
function handleStateSync(message) {
    const { version, timestamp, data } = message;
    
    if (!data) {
        console.warn('sync message missing data payload');
        return;
    }
    
    console.log(`Processing sync message (version: ${version})`);
    
    // Check for version conflicts before processing
    // Note: sync messages reset the version, so we don't reject them
    // but we do log if there was a gap
    if (version !== undefined) {
        checkVersionConflict(version);
    }
    
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
    if (data.issues !== undefined) {
        state.setEntities('issues', normalizeEntities(data.issues, 'issue'));
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
    
    // Set edges (server may send as 'edges' or 'links')
    const edgesData = data.edges !== undefined ? data.edges : data.links;
    if (edgesData !== undefined) {
        state.setEdges(normalizeEdges(edgesData));
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
    
    // Capture initial entity IDs for new-creation event detection (first sync only)
    if (!state.get('sync.initialLoadComplete')) {
        captureInitialEntityIds();
        state.set('sync.initialLoadComplete', true);
        console.log(`Initial load complete: captured ${state.get('sync.initialEntityIds').size} entity IDs`);
    }
    
    console.log(`Sync complete: ${countEntities()} entities, ${state.getEdges().length} edges`);
}

// Register for both 'sync' (legacy) and 'state' (new protocol) message types
registerHandler('sync', handleStateSync);
registerHandler('state', handleStateSync);

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
    
    // Check for version conflicts
    if (version !== undefined) {
        checkVersionConflict(version);
    }
    
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

/**
 * Handle 'log_entry' message - real-time log streaming
 * 
 * Expected message format:
 * {
 *   type: 'log_entry',
 *   entry: {
 *     timestamp: string,
 *     entity_type: string,
 *     entity_id: string,
 *     action: 'created' | 'updated' | 'closed' | 'reopened',
 *     details: string | null,
 *     actor: string | null,
 *     actor_type: 'user' | 'agent' | null
 *   },
 *   version: number
 * }
 */
registerHandler('log_entry', (message) => {
    const { entry, version } = message;
    
    if (!entry) {
        console.warn('log_entry message missing entry payload');
        return;
    }
    
    // Check for version conflicts
    if (version !== undefined) {
        checkVersionConflict(version);
        state.set('sync.version', version);
    }
    
    // Get current log entries
    const currentLog = state.get('log') || [];
    
    // Prepend new entry (newest first)
    const updatedLog = [entry, ...currentLog];
    
    // Update state (this will trigger subscribers)
    state.set('log', updatedLog);
    
    console.debug('Added log entry:', entry);
});

/**
 * Handle 'sync_catchup' message - incremental catch-up from missed messages
 * 
 * When a client reconnects or detects a version gap, it sends a request_sync
 * with last_version. If the server can provide incremental updates, it sends
 * sync_catchup with an array of missed messages to replay.
 * 
 * Expected message format:
 * {
 *   type: 'sync_catchup',
 *   version: number,
 *   messages: Array<{type: string, ...}>  // Array of missed incremental messages
 * }
 */
registerHandler('sync_catchup', (message) => {
    const { version, messages } = message;
    
    console.log(`Processing sync_catchup: ${messages?.length || 0} messages to version ${version}`);
    
    if (!Array.isArray(messages)) {
        console.warn('sync_catchup missing messages array');
        return;
    }
    
    // Process each missed message in order
    for (const msg of messages) {
        if (msg && msg.type) {
            handleMessage(msg);
        }
    }
    
    // Update version after processing all messages
    if (version !== undefined) {
        state.set('sync.version', version);
    }
    
    console.log(`Sync catch-up complete, now at version ${version}`);
});

/**
 * Handle 'sync_response' message - server response when catch-up not possible
 * 
 * When the server cannot provide incremental catch-up (e.g., client version
 * is too old or no version was provided), it sends sync_response with
 * action='reload' to tell the client to do a full state refresh.
 * 
 * Expected message format:
 * {
 *   type: 'sync_response',
 *   version: number,
 *   action: 'reload',
 *   reason: 'version_too_old' | 'no_version_provided'
 * }
 */
registerHandler('sync_response', (message) => {
    const { version, action, reason } = message;
    
    console.log(`Sync response: action=${action}, reason=${reason}, version=${version}`);
    
    if (action === 'reload') {
        // Trigger full reload via the reload callback
        // This is the fallback when incremental sync isn't possible
        if (reloadCallback) {
            reloadCallback();
        }
    }
    
    if (version !== undefined) {
        state.set('sync.version', version);
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
        state.getIssues().length +
        state.getIdeas().length +
        state.getTests().length +
        state.getDocs().length +
        state.getMilestones().length +
        state.getAgents().length
    );
}

/**
 * Capture all current entity IDs to track what existed at GUI load time
 * This enables detection of truly new entities vs filter-revealed entities.
 */
function captureInitialEntityIds() {
    const initialIds = new Set();
    
    // Collect IDs from all entity types
    const entityTypes = ['tasks', 'bugs', 'issues', 'ideas', 'tests', 'docs', 'milestones', 'queues', 'agents'];
    for (const entityType of entityTypes) {
        const entities = state.get(`entities.${entityType}`) || [];
        for (const entity of entities) {
            if (entity.id) {
                initialIds.add(entity.id);
            }
        }
    }
    
    state.set('sync.initialEntityIds', initialIds);
}

/**
 * Check if an entity ID is newly created (wasn't present at initial load)
 * @param {string} id - Entity ID to check
 * @returns {boolean} True if entity was created after GUI opened
 */
function isNewlyCreatedEntity(id) {
    // If initial load hasn't completed, treat as not new (avoid false positives)
    if (!state.get('sync.initialLoadComplete')) {
        return false;
    }
    
    const initialIds = state.get('sync.initialEntityIds');
    if (!initialIds) {
        return false;
    }
    
    return !initialIds.has(id);
}

/**
 * Trigger a follow event for a newly created entity
 * @param {string} entityType - Entity type (task, bug, idea, etc.)
 * @param {string} entityId - Entity ID
 */
function triggerFollowEvent(entityType, entityId) {
    // Check if Follow Events is enabled
    const followEvents = state.get('ui.followEvents');
    if (!followEvents) {
        console.debug(`Follow Events disabled, skipping event for ${entityType} ${entityId}`);
        return;
    }
    
    // Check if this entity type should trigger events
    const followEventsConfig = state.get('ui.followEventsConfig') || {};
    const nodeTypes = followEventsConfig.nodeTypes || {};
    
    if (!nodeTypes[entityType]) {
        console.debug(`Follow Events: ${entityType} events disabled, skipping ${entityId}`);
        return;
    }
    
    // Add event to the queue
    console.log(`Follow Events: Adding ${entityType} ${entityId} to event queue`);
    const eventQueue = state.get('ui.eventQueue') || [];
    const newEvent = {
        entityType,
        entityId,
        timestamp: Date.now()
    };
    eventQueue.push(newEvent);
    state.set('ui.eventQueue', eventQueue);
}


/**
 * Handle 'entity_added' message - incremental entity addition
 * 
 * Expected message format:
 * {
 *   type: 'entity_added',
 *   entity_type: 'task' | 'bug' | 'idea' | 'test' | 'doc' | 'milestone' | 'queue' | 'agent',
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
    
    // Check for version conflicts and skip processing if gap detected
    if (version !== undefined && !checkVersionConflict(version)) {
        console.log(`Skipping entity_added (${entity_type} ${id}) - waiting for full sync`);
        return;
    }
    
    // Update sync metadata
    state.set('sync.version', version);
    state.set('sync.lastSync', timestamp);
    
    // Map entity_type to state entities key
    const entityKey = getEntityKey(entity_type);
    if (entityKey) {
        const normalized = normalizeEntity(entity, entity_type);
        // Skip if entity was filtered out (e.g., non-worker agent)
        if (normalized !== null) {
            state.upsertEntity(entityKey, normalized);
            
            // Check if this is a newly created entity (not just revealed by filter change)
            if (isNewlyCreatedEntity(id)) {
                console.log(`Detected new entity creation: ${entity_type} ${id}`);
                triggerFollowEvent(entity_type, id);
            }
        } else {
            console.log(`Filtered out ${entity_type} ${id} (e.g., non-worker agent)`);
        }
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
    
    // Check for version conflicts and skip processing if gap detected
    if (version !== undefined && !checkVersionConflict(version)) {
        console.log(`Skipping entity_updated (${entity_type} ${id}) - waiting for full sync`);
        return;
    }
    
    // Update sync metadata
    state.set('sync.version', version);
    state.set('sync.lastSync', timestamp);
    
    // Map entity_type to state entities key
    const entityKey = getEntityKey(entity_type);
    if (entityKey) {
        const normalized = normalizeEntity(entity, entity_type);
        // Skip if entity was filtered out (e.g., non-worker agent)
        if (normalized !== null) {
            state.upsertEntity(entityKey, normalized);
        } else {
            console.log(`Filtered out ${entity_type} ${id} (e.g., non-worker agent)`);
            // Remove from state if it was previously added
            state.removeEntity(entityKey, id);
        }
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
    
    // Check for version conflicts and skip processing if gap detected
    if (version !== undefined && !checkVersionConflict(version)) {
        console.log(`Skipping entity_removed (${entity_type} ${id}) - waiting for full sync`);
        return;
    }
    
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
    
    // Check for version conflicts and skip processing if gap detected
    if (version !== undefined && !checkVersionConflict(version)) {
        console.log(`Skipping edge_added (${id}) - waiting for full sync`);
        return;
    }
    
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
    
    // Check for version conflicts and skip processing if gap detected
    if (version !== undefined && !checkVersionConflict(version)) {
        console.log(`Skipping edge_removed (${id}) - waiting for full sync`);
        return;
    }
    
    // Update sync metadata
    state.set('sync.version', version);
    state.set('sync.lastSync', timestamp);
    
    // Notify active-task-pane when working_on edges are removed.
    // This prevents "ghost" tasks from appearing in the fallback path.
    if (edge.edge_type === 'working_on') {
        import('../components/active-task-pane.js').then(module => {
            module.markRecentlyUnlinked?.(edge.target);
        }).catch(err => {
            console.debug('Could not notify active-task-pane of edge removal:', err);
        });
    }
    
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
        'issue': 'issues',
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
 * @returns {Object|null} Normalized entity, or null if entity should be filtered out
 */
function normalizeEntity(entity, type) {
    // Special handling for agents: filter to worker agents only and apply agent-specific normalization
    if (type === 'agent') {
        // Only include worker agents in the graph (consistent with initial fetch)
        if (entity.agent_type !== 'worker') {
            return null;
        }
        
        return {
            id: entity.id || `agent-${entity.pid}`,
            title: entity.name,
            short_name: entity.purpose || entity.name,
            type: 'agent',
            status: entity.status || 'unknown',
            pid: entity.pid,
            container_id: entity.container_id,
            started_at: entity.started_at,
            last_heartbeat: entity.last_heartbeat,
            // Keep original agent data for renderer (used by drawAgentLabel)
            _agent: entity
        };
    }
    
    // Default normalization for other entity types
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
