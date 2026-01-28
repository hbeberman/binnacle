/**
 * Binnacle Archive Loading Module
 * 
 * Handles loading .bng archive files via WASM module (BinnacleViewer).
 * Fetches archive, parses via WASM, extracts entities/edges/logs to state.
 */

import * as state from '../state.js';
import { ConnectionMode } from '../state.js';

// WASM module reference (initialized lazily)
let wasmModule = null;
let wasmInitPromise = null;

// BinnacleViewer instance for the current archive
let viewer = null;

/**
 * Initialize the WASM module (lazy, only called once)
 * @returns {Promise<Object>} The WASM module exports
 */
async function initWasm() {
    if (wasmModule) {
        return wasmModule;
    }
    
    if (wasmInitPromise) {
        return wasmInitPromise;
    }
    
    wasmInitPromise = (async () => {
        // Try multiple paths for the WASM module
        const wasmPaths = [
            '../../../pkg/binnacle.js',     // Development: relative to web/js/connection/
            '/pkg/binnacle.js',             // Deployed: absolute path
            './pkg/binnacle.js',            // Same directory fallback
            'https://hbeberman.github.io/binnacle/pkg/binnacle.js', // GitHub Pages release
        ];
        
        let lastError = null;
        
        for (const path of wasmPaths) {
            try {
                const module = await import(path);
                // Initialize the WASM module (loads the .wasm file)
                await module.default();
                // Call init() to set up panic hooks
                if (typeof module.init === 'function') {
                    module.init();
                }
                wasmModule = module;
                console.log(`WASM module loaded from: ${path}`);
                return wasmModule;
            } catch (e) {
                lastError = e;
                console.debug(`Failed to load WASM from ${path}:`, e.message);
            }
        }
        
        throw new Error(`Failed to load WASM module. Last error: ${lastError?.message || 'unknown'}`);
    })();
    
    return wasmInitPromise;
}

/**
 * Get the current BinnacleViewer instance
 * @returns {Object|null} The viewer instance, or null if no archive loaded
 */
export function getViewer() {
    return viewer;
}

/**
 * Get the WASM module version
 * @returns {Promise<string>} The binnacle version from WASM
 */
export async function getWasmVersion() {
    const wasm = await initWasm();
    return wasm.version();
}

/**
 * Load a .bng archive from a URL
 * 
 * Fetches the archive, parses it via BinnacleViewer, and populates the state
 * with entities, edges, and action logs.
 * 
 * @param {string} url - URL to the .bng archive file
 * @returns {Promise<Object>} Archive info { nodeCount, edgeCount, logCount, manifest }
 * @throws {Error} If fetch fails or archive is invalid
 */
export async function loadArchive(url) {
    // Initialize WASM if needed
    await initWasm();
    
    // Fetch the archive
    const response = await fetch(url);
    if (!response.ok) {
        throw new Error(`Failed to fetch archive: HTTP ${response.status} ${response.statusText}`);
    }
    
    const arrayBuffer = await response.arrayBuffer();
    const bytes = new Uint8Array(arrayBuffer);
    
    // Parse via WASM
    return loadArchiveFromBytes(bytes, url);
}

/**
 * Load a .bng archive from a File object (for drag-drop or file input)
 * 
 * @param {File} file - The File object to load
 * @returns {Promise<Object>} Archive info { nodeCount, edgeCount, logCount, manifest }
 * @throws {Error} If file read fails or archive is invalid
 */
export async function loadArchiveFromFile(file) {
    // Initialize WASM if needed
    await initWasm();
    
    const arrayBuffer = await file.arrayBuffer();
    const bytes = new Uint8Array(arrayBuffer);
    
    return loadArchiveFromBytes(bytes, file.name);
}

/**
 * Load archive from raw bytes
 * 
 * @param {Uint8Array} bytes - The raw archive bytes
 * @param {string} source - Source identifier (URL or filename) for display
 * @returns {Promise<Object>} Archive info
 */
async function loadArchiveFromBytes(bytes, source) {
    const wasm = await initWasm();
    
    // Create a new viewer instance (free old one if exists)
    if (viewer) {
        viewer.free();
        viewer = null;
    }
    
    viewer = new wasm.BinnacleViewer();
    
    // Load the archive data
    viewer.loadFromBytes(bytes);
    
    // Extract data from viewer
    const nodesJson = viewer.getNodesJson();
    const edgesJson = viewer.getEdgesJson();
    const logsJson = viewer.getActionLogsJson();
    const manifestJson = viewer.getManifestJson();
    
    const nodes = JSON.parse(nodesJson);
    const edges = JSON.parse(edgesJson);
    const logs = JSON.parse(logsJson);
    const manifest = JSON.parse(manifestJson);
    
    // Categorize entities by type
    const entities = categorizeEntities(nodes);
    
    // Transform edges to state format
    const stateEdges = transformEdges(edges);
    
    // Update global state
    state.setMode(ConnectionMode.ARCHIVE, { archiveUrl: source });
    
    // Populate entities
    state.setEntities('tasks', entities.tasks);
    state.setEntities('bugs', entities.bugs);
    state.setEntities('ideas', entities.ideas);
    state.setEntities('tests', entities.tests);
    state.setEntities('docs', entities.docs);
    state.setEntities('milestones', entities.milestones);
    state.setEntities('queues', entities.queues);
    state.setEntities('agents', entities.agents);
    
    // Set edges
    state.setEdges(stateEdges);
    
    // Set action logs
    state.set('log', logs);
    state.set('logPagination.total', logs.length);
    state.set('logPagination.hasMore', false); // Archives have all logs loaded
    
    // Store manifest metadata
    state.set('sync.lastSync', manifest.exported_at || null);
    
    return {
        nodeCount: nodes.length,
        edgeCount: edges.length,
        logCount: logs.length,
        manifest,
        source
    };
}

/**
 * Categorize nodes by entity type
 * 
 * @param {Array} nodes - Array of node objects from WASM
 * @returns {Object} Categorized entities { tasks, bugs, ideas, tests, docs, milestones, queues, agents }
 */
function categorizeEntities(nodes) {
    const entities = {
        tasks: [],
        bugs: [],
        ideas: [],
        tests: [],
        docs: [],
        milestones: [],
        queues: [],
        agents: []
    };
    
    for (const node of nodes) {
        // Map node type to entity category
        const type = node.type?.toLowerCase() || 'task';
        
        // Transform node to entity format expected by state
        const entity = transformNodeToEntity(node);
        
        switch (type) {
            case 'task':
                entities.tasks.push(entity);
                break;
            case 'bug':
                entities.bugs.push(entity);
                break;
            case 'idea':
                entities.ideas.push(entity);
                break;
            case 'test':
                entities.tests.push(entity);
                break;
            case 'doc':
                entities.docs.push(entity);
                break;
            case 'milestone':
                entities.milestones.push(entity);
                break;
            case 'queue':
                entities.queues.push(entity);
                break;
            case 'agent':
                entities.agents.push(entity);
                break;
            default:
                // Unknown types go to tasks as fallback
                console.warn(`Unknown node type: ${type}, treating as task`);
                entities.tasks.push(entity);
        }
    }
    
    return entities;
}

/**
 * Transform a WASM node to state entity format
 * 
 * @param {Object} node - Node from WASM getNodesJson()
 * @returns {Object} Entity in state format
 */
function transformNodeToEntity(node) {
    // Base entity fields
    const entity = {
        id: node.id,
        type: node.type,
        title: node.title || node.name || node.id,
        short_name: node.short_name || null,
        status: node.status || 'pending',
        priority: node.priority ?? 2,
        tags: node.tags || [],
        created_at: node.created_at || null,
        updated_at: node.updated_at || null,
        closed_at: node.closed_at || null,
        // Layout position from WASM (for archive mode)
        _layout: {
            x: node.x,
            y: node.y
        }
    };
    
    // Type-specific fields
    if (node.description !== undefined) {
        entity.description = node.description;
    }
    if (node.assignee !== undefined) {
        entity.assignee = node.assignee;
    }
    if (node.severity !== undefined) {
        entity.severity = node.severity;
    }
    if (node.due_date !== undefined) {
        entity.due_date = node.due_date;
    }
    if (node.doc_type !== undefined) {
        entity.doc_type = node.doc_type;
    }
    if (node.content !== undefined) {
        entity.content = node.content;
    }
    if (node.command !== undefined) {
        entity.command = node.command;
    }
    if (node.working_dir !== undefined) {
        entity.working_dir = node.working_dir;
    }
    
    return entity;
}

/**
 * Transform WASM edges to state edge format
 * 
 * @param {Array} edges - Edges from WASM getEdgesJson()
 * @returns {Array} Edges in state format
 */
function transformEdges(edges) {
    return edges.map(edge => ({
        source: edge.source,
        target: edge.target,
        edge_type: edge.edge_type || edge.type || 'related_to',
        // Store layout positions for archive rendering
        _layout: {
            source_x: edge.source_x,
            source_y: edge.source_y,
            target_x: edge.target_x,
            target_y: edge.target_y
        }
    }));
}

/**
 * Run layout algorithm on the loaded archive
 * 
 * @param {number} maxIterations - Maximum iterations to run (default 500)
 * @param {Function} onProgress - Optional callback with progress (0-100)
 * @returns {Promise<boolean>} True if layout converged, false if max iterations reached
 */
export async function runLayout(maxIterations = 500, onProgress = null) {
    if (!viewer) {
        throw new Error('No archive loaded');
    }
    
    const batchSize = 50;
    let iterations = 0;
    
    while (iterations < maxIterations && !viewer.isStable()) {
        viewer.runLayout(batchSize);
        iterations += batchSize;
        
        if (onProgress) {
            const progress = Math.min(100, Math.round((iterations / maxIterations) * 100));
            onProgress(progress);
        }
        
        // Yield to UI thread
        await new Promise(resolve => setTimeout(resolve, 0));
    }
    
    // Update node positions in state after layout
    updateNodePositionsFromViewer();
    
    return viewer.isStable();
}

/**
 * Update state entity positions from viewer layout
 */
function updateNodePositionsFromViewer() {
    if (!viewer) return;
    
    const nodesJson = viewer.getNodesJson();
    const nodes = JSON.parse(nodesJson);
    
    // Create position lookup
    const positions = new Map();
    for (const node of nodes) {
        positions.set(node.id, { x: node.x, y: node.y });
    }
    
    // Update each entity type
    for (const type of ['tasks', 'bugs', 'ideas', 'tests', 'docs', 'milestones', 'queues', 'agents']) {
        const entities = state.get(`entities.${type}`) || [];
        const updated = entities.map(entity => {
            const pos = positions.get(entity.id);
            if (pos) {
                return { ...entity, _layout: pos };
            }
            return entity;
        });
        state.setEntities(type, updated);
    }
    
    // Update edges with positions
    const edgesJson = viewer.getEdgesJson();
    const edges = JSON.parse(edgesJson);
    state.setEdges(transformEdges(edges));
}

/**
 * Check if layout is ready/stable
 * @returns {boolean}
 */
export function isLayoutReady() {
    return viewer ? viewer.isLayoutReady() : false;
}

/**
 * Check if layout has converged
 * @returns {boolean}
 */
export function isLayoutStable() {
    return viewer ? viewer.isStable() : false;
}

/**
 * Get viewport/camera info from viewer
 * @returns {Object} { x, y, zoom }
 */
export function getViewport() {
    if (!viewer) {
        return { x: 0, y: 0, zoom: 1.0 };
    }
    return {
        x: viewer.getCameraX(),
        y: viewer.getCameraY(),
        zoom: viewer.getZoom()
    };
}

/**
 * Set viewport/camera
 * @param {number} x - Camera X position
 * @param {number} y - Camera Y position
 * @param {number} zoom - Zoom level
 */
export function setViewport(x, y, zoom) {
    if (!viewer) return;
    
    // Pan to position (viewer.pan takes delta, not absolute)
    const currentX = viewer.getCameraX();
    const currentY = viewer.getCameraY();
    viewer.pan(x - currentX, y - currentY);
    viewer.setZoom(zoom);
}

/**
 * Focus on a specific node
 * @param {string} nodeId - Node ID to focus
 */
export function focusNode(nodeId) {
    if (!viewer) return;
    viewer.focusNode(nodeId);
    
    // Update state viewport after focus
    state.setViewport({
        panX: viewer.getCameraX(),
        panY: viewer.getCameraY(),
        zoom: viewer.getZoom()
    });
}

/**
 * Find node at screen coordinates
 * @param {number} screenX - Screen X coordinate
 * @param {number} screenY - Screen Y coordinate
 * @returns {string|null} Node ID or null
 */
export function findNodeAt(screenX, screenY) {
    if (!viewer) return null;
    return viewer.findNodeAt(screenX, screenY);
}

/**
 * Clean up resources
 */
export function dispose() {
    if (viewer) {
        viewer.free();
        viewer = null;
    }
}
