/**
 * Binnacle Graph - Canvas Renderer
 * 
 * Main rendering module for the graph canvas. Handles:
 * - Canvas setup and resize
 * - Node and edge drawing
 * - Animation loop
 * - Visual effects (selection, hover, animations)
 */

import * as state from '../state.js';
import { ConnectionStatus } from '../state.js';
import { drawNodeShapePath } from './shapes.js';
import { getNodeColor, getEdgeStyle, getCSSColors } from './colors.js';
import { worldToScreen, screenToWorld, getZoom, centerOn, panToNode } from './transform.js';
import * as camera from './camera.js';
import { getRevealOpacity } from '../utils/reveal-animation.js';
import { getFadeOutOpacity } from '../utils/family-collapse.js';

// Animation constants
const AGENT_DEPARTURE_FADE_MS = 5000;
const NEW_BADGE_DURATION_MS = 10000; // NEW badge fades after 10 seconds
const NEW_BADGE_FADE_IN_MS = 300; // NEW badge fade-in duration
const NEW_BADGE_FADE_OUT_MS = 2000; // NEW badge fade-out duration

// Easing functions for smooth animations
/**
 * Ease-in quadratic - starts slow, accelerates
 * Good for fade-out effects
 */
function easeInQuad(t) {
    return t * t;
}

/**
 * Ease-out quadratic - starts fast, decelerates
 * Good for fade-in effects
 */
function easeOutQuad(t) {
    return t * (2 - t);
}

// Renderer state
let canvas = null;
let ctx = null;
let animationFrameId = null;
let isAnimating = false;
let animationTime = 0;

// Graph data (cached for rendering)
let graphNodes = [];
let graphEdges = [];
let visibleNodes = [];
let visibleNodeIds = new Set();

// Interaction state
let hoveredNode = null;
let selectedNodes = []; // Multi-selection array
let draggedNode = null;

// Selection animation tracking (for pulse effect on new selections)
const selectionAnimations = new Map(); // nodeId -> timestamp when selected
const SELECTION_ANIMATION_DURATION = 600; // ms

// Departing agents tracking (for fade animation)
const departingAgents = new Map();

// Highlight state (for programmatic highlighting, e.g., from entity links)
let highlightedNodeId = null;
const HIGHLIGHT_ANIMATION_DURATION = 2000; // ms - longer than selection for visibility
let highlightStartTime = null;

// Agent status tracking (for detecting when agents acquire work)
const previousAgentStatuses = new Map(); // agentId -> previous status

// Goodbye linger pause tracking for visibility changes
let goodbyePauseStartTime = null;
let goodbyeRemainingTime = null;

/**
 * Initialize the graph renderer with a canvas element
 * @param {HTMLCanvasElement} canvasElement - The canvas to render to
 * @param {Object} callbacks - Optional callback functions
 * @param {Function} callbacks.onNodeClick - Called when single-clicking a node
 * @param {Function} callbacks.onNodeDoubleClick - Called when double-clicking a node
 * @param {Function} callbacks.onCanvasClick - Called when clicking on empty space
 */
export function init(canvasElement, callbacks = {}) {
    canvas = canvasElement;
    ctx = canvas.getContext('2d');
    
    // Set initial canvas size
    resizeCanvas();
    
    // Initialize camera controls (panning, zooming, node dragging, hover)
    camera.init(canvasElement, {
        onNodeClick: callbacks.onNodeClick || null,
        onNodeDoubleClick: callbacks.onNodeDoubleClick || null,
        onCanvasClick: callbacks.onCanvasClick || null
    });
    
    // Subscribe to state changes that require re-render
    state.subscribe('entities.*', onEntitiesChanged);
    state.subscribe('edges', onEdgesChanged);
    state.subscribe('ui.viewport', scheduleRender);
    state.subscribe('ui.hideCompleted', scheduleRender);
    state.subscribe('ui.searchQuery', scheduleRender);
    state.subscribe('ui.nodeTypeFilters', scheduleRender);
    state.subscribe('ui.edgeTypeFilters', scheduleRender);
    state.subscribe('ui.selectedNode', onSelectionChanged);
    state.subscribe('ui.selectedNodes', onMultiSelectionChanged);
    state.subscribe('ui.boxSelection', scheduleRender);
    
    // Handle page visibility changes (tab switch, minimize) for goodbye linger
    document.addEventListener('visibilitychange', handleGoodbyeVisibilityChange);
    
    // Build initial graph from current state (if any entities already loaded)
    buildGraphNodes();
    buildGraphEdges();
    filterVisibleNodes();
    
    console.log(`[Graph] Initialized with ${graphNodes.length} nodes, ${visibleNodes.length} visible`);
    
    // Render initial state
    if (visibleNodes.length > 0) {
        startAnimation();
    }
}

/**
 * Resize the canvas to match its display size
 */
export function resizeCanvas() {
    if (!canvas) return;
    
    const displayWidth = canvas.clientWidth;
    const displayHeight = canvas.clientHeight;
    
    if (canvas.width !== displayWidth || canvas.height !== displayHeight) {
        canvas.width = displayWidth;
        canvas.height = displayHeight;
        scheduleRender();
    }
}

/**
 * Get initial position for a new node
 * Places it near connected nodes if edges exist, or near viewport center
 * @param {string} nodeId - ID of the new node
 * @param {Map} existingNodes - Map of existing nodes by ID
 * @returns {Object} { x, y } position in world coordinates
 */
function getInitialPositionForNewNode(nodeId, existingNodes) {
    // Check if this node has a spawn position from family reveal
    const familyReveal = state.get('ui.familyReveal');
    if (familyReveal && familyReveal.active && familyReveal.spawnPositions) {
        // Ensure spawnPositions is a Map (defensive check)
        if (!(familyReveal.spawnPositions instanceof Map)) {
            console.warn('[Renderer] familyReveal.spawnPositions is not a Map, converting:', familyReveal.spawnPositions);
            const spawnMap = new Map(Object.entries(familyReveal.spawnPositions || {}));
            familyReveal.spawnPositions = spawnMap;
            // Persist the conversion back to state
            state.set('ui.familyReveal.spawnPositions', spawnMap);
        }
        
        const spawnPos = familyReveal.spawnPositions.get(nodeId);
        if (spawnPos) {
            console.log(`Using spawn position for ${nodeId}: (${spawnPos.x.toFixed(1)}, ${spawnPos.y.toFixed(1)})`);
            return { x: spawnPos.x, y: spawnPos.y };
        }
    }
    
    // Get all edges from state
    const edges = state.get('edges') || [];
    
    // Find edges connected to this new node
    const connectedEdges = edges.filter(e => e.source === nodeId || e.target === nodeId);
    
    if (connectedEdges.length > 0) {
        // Place near connected nodes (average position with some randomness)
        let sumX = 0;
        let sumY = 0;
        let count = 0;
        
        for (const edge of connectedEdges) {
            const connectedId = edge.source === nodeId ? edge.target : edge.source;
            const connectedNode = existingNodes.get(connectedId);
            
            if (connectedNode) {
                sumX += connectedNode.x;
                sumY += connectedNode.y;
                count++;
            }
        }
        
        if (count > 0) {
            // Average position with random offset to avoid exact overlap
            const avgX = sumX / count;
            const avgY = sumY / count;
            const offsetAngle = Math.random() * 2 * Math.PI;
            const offsetRadius = 80 + Math.random() * 40; // 80-120 units away
            
            return {
                x: avgX + Math.cos(offsetAngle) * offsetRadius,
                y: avgY + Math.sin(offsetAngle) * offsetRadius
            };
        }
    }
    
    // No connected nodes or no existing connections found
    // Place at a random position near the viewport center
    const viewport = state.get('ui.viewport');
    const centerX = -viewport.panX / viewport.zoom;
    const centerY = -viewport.panY / viewport.zoom;
    
    // Add some randomness to avoid stacking
    const randomAngle = Math.random() * 2 * Math.PI;
    const randomRadius = Math.random() * 150; // Within 150 units of center
    
    return {
        x: centerX + Math.cos(randomAngle) * randomRadius,
        y: centerY + Math.sin(randomAngle) * randomRadius
    };
}

/**
 * Build graph nodes from entities
 */
function buildGraphNodes() {
    const existingNodes = new Map(graphNodes.map(n => [n.id, n]));
    
    // Debug: check if we're using the same state instance
    if (typeof state._getStateObjectRef === 'function') {
        const stateRef = state._getStateObjectRef();
        console.log(`[Graph.buildGraphNodes] State object ID: ${stateRef && stateRef.entities ? 'valid' : 'INVALID'}`);
    }
    
    // Combine all entity types
    const tasks = state.get('entities.tasks') || [];
    const bugs = state.get('entities.bugs') || [];
    const ideas = state.get('entities.ideas') || [];
    const tests = state.get('entities.tests') || [];
    const docs = state.get('entities.docs') || [];
    const milestones = state.get('entities.milestones') || [];
    const queues = state.get('entities.queues') || [];
    const agents = state.get('entities.agents') || [];
    
    console.log(`[Graph] buildGraphNodes called: tasks=${tasks.length}, bugs=${bugs.length}, ideas=${ideas.length}, tests=${tests.length}, docs=${docs.length}, milestones=${milestones.length}, queues=${queues.length}, agents=${agents.length}`);
    
    const allEntities = [
        ...tasks,
        ...bugs,
        ...ideas,
        ...tests,
        ...docs,
        ...milestones,
        ...queues,
        ...agents
    ];
    
    graphNodes = allEntities.map((entity) => {
        const existing = existingNodes.get(entity.id);
        
        // For agent nodes, store the agent data in _agent for label rendering
        const agentData = entity.type === 'agent' ? entity : entity._agent;
        
        // Initialize agent status tracking for new agents
        if (entity.type === 'agent' && !previousAgentStatuses.has(entity.id)) {
            previousAgentStatuses.set(entity.id, (entity.status || 'unknown').toLowerCase());
        }
        
        if (existing) {
            // Preserve position, update data
            return {
                ...existing,
                title: entity.title,
                short_name: entity.short_name,
                status: entity.status,
                priority: entity.priority,
                type: entity.type || 'task',
                closed_at: entity.closed_at,
                doc_type: entity.doc_type,
                _departing: entity._departing,
                _agent: agentData,
                vx: 0,  // Reset velocity to prevent oscillation
                vy: 0
            };
        } else {
            // New node - place it near connected nodes if any, else near viewport center
            const { x: initialX, y: initialY } = getInitialPositionForNewNode(entity.id, existingNodes);
            
            return {
                id: entity.id,
                title: entity.title,
                short_name: entity.short_name,
                status: entity.status,
                priority: entity.priority,
                type: entity.type || 'task',
                closed_at: entity.closed_at,
                doc_type: entity.doc_type,
                _departing: entity._departing,
                _agent: agentData,
                x: initialX,
                y: initialY,
                vx: 0,
                vy: 0,
                radius: 30  // Node radius in world units
            };
        }
    });
    
    // Clean up status tracking for removed agents
    const currentAgentIds = new Set(agents.map(a => a.id));
    for (const agentId of previousAgentStatuses.keys()) {
        if (!currentAgentIds.has(agentId)) {
            previousAgentStatuses.delete(agentId);
        }
    }
}

/**
 * Build graph edges from state
 */
function buildGraphEdges() {
    const edges = state.get('edges') || [];
    graphEdges = edges.map(edge => ({
        from: edge.source,
        to: edge.target,
        edge_type: edge.edge_type,
        bidirectional: edge.bidirectional,
        reason: edge.reason
    }));
}

/**
 * Filter visible nodes based on settings
 */
function filterVisibleNodes() {
    const hideCompleted = state.get('ui.hideCompleted');
    const nodeFilters = state.get('ui.nodeTypeFilters') || {};
    const searchQuery = (state.get('ui.searchQuery') || '').toLowerCase().trim();
    const familyReveal = state.get('ui.familyReveal') || { active: false, revealedNodeIds: new Set() };
    
    // Ensure revealedNodeIds is a Set (defensive check)
    if (familyReveal.revealedNodeIds && !(familyReveal.revealedNodeIds instanceof Set)) {
        console.warn('[Renderer] familyReveal.revealedNodeIds is not a Set, converting:', familyReveal.revealedNodeIds);
        const revealedSet = new Set(familyReveal.revealedNodeIds);
        familyReveal.revealedNodeIds = revealedSet;
        // Persist the conversion back to state to prevent repeated conversions
        state.set('ui.familyReveal.revealedNodeIds', revealedSet);
    }
    
    visibleNodes = graphNodes.filter(node => {
        // Always include nodes revealed by family reveal
        if (familyReveal.active && familyReveal.revealedNodeIds && familyReveal.revealedNodeIds.has(node.id)) {
            return true;
        }
        
        // Apply node type filter
        if (nodeFilters[node.type] === false) return false;
        
        // Apply hide completed filter
        if (hideCompleted && (node.status === 'done' || node.status === 'cancelled')) {
            return false;
        }
        
        // Apply search filter
        if (searchQuery) {
            const matchesId = node.id.toLowerCase().includes(searchQuery);
            const matchesTitle = (node.title || '').toLowerCase().includes(searchQuery);
            const matchesShortName = (node.short_name || '').toLowerCase().includes(searchQuery);
            if (!matchesId && !matchesTitle && !matchesShortName) {
                return false;
            }
        }
        
        return true;
    });
    
    visibleNodeIds = new Set(visibleNodes.map(n => n.id));
}

/**
 * Rebuild graph from current state data
 * Public API for manually triggering graph rebuild
 */
export function rebuildGraph() {
    console.log(`[Graph] Manual rebuild triggered, nodes before: ${graphNodes.length}`);
    buildGraphNodes();
    buildGraphEdges();
    filterVisibleNodes();
    console.log(`[Graph] After manual rebuild: ${graphNodes.length} nodes, ${visibleNodes.length} visible`);
    startAnimation();
}

/**
 * Handle entity changes
 */
function onEntitiesChanged(newValue, oldValue, path) {
    console.log(`[Graph] Entity changed: ${path}`, { nodesBefore: graphNodes.length, visibleBefore: visibleNodes.length });
    buildGraphNodes();
    filterVisibleNodes();
    console.log(`[Graph] After rebuild: ${graphNodes.length} nodes, ${visibleNodes.length} visible`);
    startAnimation();
}

/**
 * Handle edge changes
 */
function onEdgesChanged() {
    buildGraphEdges();
    scheduleRender();
}

/**
 * Handle single selection changes (backward compatibility)
 */
function onSelectionChanged(nodeId) {
    // Keep for backward compatibility with components that use ui.selectedNode
    scheduleRender();
    
    // Clear NEW badge when node gets selected
    if (nodeId) {
        const newBadges = state.get('ui.newBadges') || new Map();
        if (newBadges.has(nodeId)) {
            newBadges.delete(nodeId);
            state.set('ui.newBadges', newBadges);
        }
    }
}

/**
 * Handle multi-selection changes
 */
function onMultiSelectionChanged(nodeIds) {
    const previousSelection = new Set(selectedNodes);
    selectedNodes = nodeIds || [];
    
    // Track newly selected nodes for animation
    const now = performance.now();
    for (const nodeId of selectedNodes) {
        if (!previousSelection.has(nodeId)) {
            selectionAnimations.set(nodeId, now);
        }
    }
    
    // Clean up animations for deselected nodes
    for (const [nodeId] of selectionAnimations) {
        if (!selectedNodes.includes(nodeId)) {
            selectionAnimations.delete(nodeId);
        }
    }
    
    // Clear NEW badges for all selected nodes
    const newBadges = state.get('ui.newBadges') || new Map();
    let badgesChanged = false;
    for (const nodeId of selectedNodes) {
        if (newBadges.has(nodeId)) {
            newBadges.delete(nodeId);
            badgesChanged = true;
        }
    }
    if (badgesChanged) {
        state.set('ui.newBadges', newBadges);
    }
    
    startAnimation(); // Ensure animation runs for selection effects
    scheduleRender();
}

/**
 * Schedule a single render frame
 */
function scheduleRender() {
    if (!isAnimating) {
        requestAnimationFrame(() => render());
    }
}

/**
 * Start the animation loop
 */
export function startAnimation() {
    if (!isAnimating) {
        isAnimating = true;
        animationFrameId = requestAnimationFrame(animate);
    }
}

/**
 * Stop the animation loop
 */
export function stopAnimation() {
    isAnimating = false;
    if (animationFrameId) {
        cancelAnimationFrame(animationFrameId);
        animationFrameId = null;
    }
}

/**
 * Apply force-directed layout physics to graph nodes
 */
function applyPhysics() {
    const physics = state.getState().ui.physics;
    
    // Reset forces
    for (const node of visibleNodes) {
        node.fx = 0;
        node.fy = 0;
    }
    
    // Node-node repulsion (all pairs)
    for (let i = 0; i < visibleNodes.length; i++) {
        for (let j = i + 1; j < visibleNodes.length; j++) {
            const a = visibleNodes[i];
            const b = visibleNodes[j];
            
            const dx = b.x - a.x;
            const dy = b.y - a.y;
            const distSq = dx * dx + dy * dy;
            
            // Skip if nodes are at exactly the same position
            if (distSq === 0) continue;
            
            const dist = Math.sqrt(distSq);
            const force = physics.repulsionStrength / distSq;
            const fx = (dx / dist) * force;
            const fy = (dy / dist) * force;
            
            a.fx -= fx;
            a.fy -= fy;
            b.fx += fx;
            b.fy += fy;
        }
    }
    
    // Edge attraction (spring-based with resting length)
    const visibleNodeMap = new Map(visibleNodes.map(n => [n.id, n]));
    const physicsFilters = state.get('ui.edgePhysicsFilters') || {};
    
    for (const edge of graphEdges) {
        // Skip edges that have physics disabled
        if (physicsFilters[edge.edge_type] === false) continue;
        
        const source = visibleNodeMap.get(edge.from);
        const target = visibleNodeMap.get(edge.to);
        
        if (!source || !target) continue;
        
        const dx = target.x - source.x;
        const dy = target.y - source.y;
        const distance = Math.sqrt(dx * dx + dy * dy);
        
        if (distance === 0) continue;
        
        // Spring force: pulls nodes toward resting length
        // - If compressed (distance < resting): pushes apart
        // - If extended (distance > resting): pulls together
        // Use edge-type-specific resting length if defined, otherwise use default
        const edgeRestingLengths = physics.edgeRestingLengths || {};
        const restingLength = edgeRestingLengths[edge.edge_type] ?? physics.springRestingLength;
        const deviation = distance - restingLength;
        const force = physics.springStrength * deviation;
        const fx = (dx / distance) * force;
        const fy = (dy / distance) * force;
        
        source.fx += fx;
        source.fy += fy;
        target.fx -= fx;
        target.fy -= fy;
    }
    
    // Center gravity
    for (const node of visibleNodes) {
        // Queue nodes are HEAVY - they barely respond to gravity
        const gravityMultiplier = node.type === 'queue' ? 0.1 : 1.0;
        node.fx -= node.x * physics.gravityStrength * gravityMultiplier;
        node.fy -= node.y * physics.gravityStrength * gravityMultiplier;
    }
    
    // Update velocities and positions
    for (const node of visibleNodes) {
        // Skip dragged nodes
        if (node === draggedNode) continue;
        
        node.vx = (node.vx + node.fx) * physics.damping;
        node.vy = (node.vy + node.fy) * physics.damping;
        
        // Max velocity (queue nodes move slower)
        const maxVelocity = node.type === 'queue' ? 0.9 : 3.0;
        const speed = Math.sqrt(node.vx * node.vx + node.vy * node.vy);
        if (speed > maxVelocity) {
            const scale = maxVelocity / speed;
            node.vx *= scale;
            node.vy *= scale;
        }
        
        node.x += node.vx;
        node.y += node.vy;
    }
}

/**
 * Update auto-follow camera behavior
 * Pans the camera to follow nodes based on user configuration
 */
function updateAutoFollow() {
    const autoFollow = state.get('ui.autoFollow');
    const userPaused = state.get('ui.userPaused');
    const currentView = state.get('ui.currentView');
    
    // Don't auto-follow if disabled or user has paused it
    if (!autoFollow || userPaused) {
        return;
    }
    
    // Only perform auto-follow when on the Graph tab
    // Follow mode should never pull users to Graph from other tabs
    if (currentView !== 'graph') {
        return;
    }
    
    // Get follow type filter ('' = any, 'task', 'bug', 'idea', 'agent')
    const followTypeFilter = state.get('ui.followTypeFilter') || '';
    
    // Filter nodes based on type selection
    let candidateNodes;
    if (followTypeFilter === '') {
        // "Any" - follow agents, tasks, bugs, ideas
        candidateNodes = visibleNodes.filter(node => 
            node.type === 'agent' || 
            node.type === 'task' || 
            node.type === 'bug' || 
            node.type === 'idea'
        );
    } else {
        // Specific type selected
        candidateNodes = visibleNodes.filter(node => node.type === followTypeFilter);
    }
    
    if (candidateNodes.length === 0) {
        return;
    }
    
    // When following agents, check for newly active agents (just acquired work)
    let newlyActiveAgent = null;
    if (followTypeFilter === 'agent' || followTypeFilter === '') {
        const agents = candidateNodes.filter(node => node.type === 'agent');
        for (const agent of agents) {
            const previousStatus = previousAgentStatuses.get(agent.id);
            const currentStatus = (agent.status || 'unknown').toLowerCase();
            
            // Detect transition to active status (agent just acquired work)
            if (currentStatus === 'active' && previousStatus && previousStatus !== 'active') {
                newlyActiveAgent = agent;
                console.log(`Agent ${agent.id} just became active - snapping to it`);
                break;
            }
            
            // Detect agent entering goodbye state
            const currentAction = (agent.current_action || '').toLowerCase();
            if (currentAction === 'goodbye') {
                const currentGoodbyeActive = state.get('ui.agentGoodbyeActive');
                if (currentGoodbyeActive !== agent.id) {
                    state.set('ui.agentGoodbyeActive', agent.id);
                    state.set('ui.goodbyeStartTime', Date.now());
                    console.log(`Agent ${agent.id} entered goodbye state - camera will linger for 3s`);
                }
            }
            
            // Update status tracking
            previousAgentStatuses.set(agent.id, currentStatus);
        }
        
        // Clear goodbye state if the tracked agent is no longer in goodbye mode
        const currentGoodbyeActive = state.get('ui.agentGoodbyeActive');
        if (currentGoodbyeActive) {
            const goodbyeAgent = agents.find(a => a.id === currentGoodbyeActive);
            if (!goodbyeAgent || (goodbyeAgent.current_action || '').toLowerCase() !== 'goodbye') {
                state.set('ui.agentGoodbyeActive', null);
                state.set('ui.goodbyeStartTime', null);
                console.log(`Agent ${currentGoodbyeActive} left goodbye state`);
            }
        }
    }
    
    // Check if we're in goodbye linger period (3 seconds after goodbye started)
    const goodbyeAgentId = state.get('ui.agentGoodbyeActive');
    const goodbyeStartTime = state.get('ui.goodbyeStartTime');
    const GOODBYE_LINGER_MS = 3000; // 3 seconds
    
    if (goodbyeAgentId && goodbyeStartTime) {
        const elapsedMs = Date.now() - goodbyeStartTime;
        if (elapsedMs < GOODBYE_LINGER_MS) {
            // Still within linger period - keep camera on goodbye agent
            const goodbyeAgent = candidateNodes.find(n => n.id === goodbyeAgentId);
            if (goodbyeAgent) {
                // Force following the goodbye agent
                const currentFollowingId = state.get('ui.followingNodeId');
                if (currentFollowingId !== goodbyeAgent.id) {
                    state.set('ui.followingNodeId', goodbyeAgent.id);
                    console.log(`Lingering on goodbye agent ${goodbyeAgent.id} (${Math.round((GOODBYE_LINGER_MS - elapsedMs) / 1000)}s remaining)`);
                }
                
                // Keep camera centered on goodbye agent
                if (canvas && goodbyeAgent.x !== undefined && goodbyeAgent.y !== undefined) {
                    centerOn(goodbyeAgent.x, goodbyeAgent.y);
                }
                return; // Don't process normal follow logic during linger
            } else {
                // Goodbye agent disappeared during linger - immediately transition to next agent
                console.log(`Goodbye agent ${goodbyeAgentId} disappeared during linger - transitioning early`);
                
                // Find next agent to follow
                const remainingAgents = candidateNodes.filter(n => 
                    n.type === 'agent' && n.id !== goodbyeAgentId
                );
                
                if (remainingAgents.length > 0) {
                    const sortedAgents = [...remainingAgents].sort((a, b) => {
                        const aTime = new Date(a.started_at || 0).getTime();
                        const bTime = new Date(b.started_at || 0).getTime();
                        return bTime - aTime;
                    });
                    
                    const nextAgent = sortedAgents[0];
                    console.log(`Transitioning to ${nextAgent.id} after goodbye agent disappeared`);
                    
                    state.set('ui.followingNodeId', nextAgent.id);
                    
                    if (canvas && nextAgent.x !== undefined && nextAgent.y !== undefined) {
                        panToNode(nextAgent.x, nextAgent.y, {
                            canvas: canvas,
                            duration: 800,
                            nodeId: nextAgent.id // Validate node during pan
                        });
                    }
                }
                
                // Clear goodbye state
                state.set('ui.agentGoodbyeActive', null);
                state.set('ui.goodbyeStartTime', null);
                goodbyePauseStartTime = null;
                goodbyeRemainingTime = null;
                return;
            }
        } else {
            // Linger period expired - transition to next agent
            console.log(`Goodbye linger period expired for ${goodbyeAgentId}`);
            
            // Find next agent to follow (by priority, excluding the goodbye agent)
            const remainingAgents = candidateNodes.filter(n => 
                n.type === 'agent' && n.id !== goodbyeAgentId
            );
            
            if (remainingAgents.length > 0) {
                // Sort agents by started_at (most recent first)
                const sortedAgents = [...remainingAgents].sort((a, b) => {
                    const aTime = new Date(a.started_at || 0).getTime();
                    const bTime = new Date(b.started_at || 0).getTime();
                    return bTime - aTime;
                });
                
                const nextAgent = sortedAgents[0];
                console.log(`Smoothly transitioning from ${goodbyeAgentId} to ${nextAgent.id}`);
                
                // Update following state
                state.set('ui.followingNodeId', nextAgent.id);
                
                // Smooth pan to next agent
                if (canvas && nextAgent.x !== undefined && nextAgent.y !== undefined) {
                    panToNode(nextAgent.x, nextAgent.y, {
                        canvas: canvas,
                        duration: 800, // Smooth 0.8-second transition
                        nodeId: nextAgent.id // Validate node during pan
                    });
                }
            } else {
                console.log(`No remaining agents - camera stays at current position`);
            }
            
            // Clear goodbye state
            state.set('ui.agentGoodbyeActive', null);
            state.set('ui.goodbyeStartTime', null);
            goodbyePauseStartTime = null;
            goodbyeRemainingTime = null;
            return; // Exit after handling goodbye transition
        }
    }
    
    // Check for pinned agent (takes priority after goodbye linger)
    const pinnedAgentId = state.get('ui.pinnedAgentId');
    if (pinnedAgentId) {
        const pinnedAgent = candidateNodes.find(n => n.id === pinnedAgentId);
        if (pinnedAgent) {
            // Pinned agent exists - follow it exclusively
            const currentFollowingId = state.get('ui.followingNodeId');
            if (currentFollowingId !== pinnedAgent.id) {
                state.set('ui.followingNodeId', pinnedAgent.id);
                console.log(`Following pinned agent ${pinnedAgent.id}`);
            }
            
            // Keep camera centered on pinned agent
            if (canvas && pinnedAgent.x !== undefined && pinnedAgent.y !== undefined) {
                centerOn(pinnedAgent.x, pinnedAgent.y);
            }
            return; // Don't process normal follow logic when pinned
        } else {
            // Pinned agent no longer exists - clear pin and fall back to auto-priority
            state.set('ui.pinnedAgentId', null);
            console.log(`Pinned agent ${pinnedAgentId} no longer exists - falling back to auto-priority`);
        }
    }
    
    // Determine target node: newly active agent takes priority
    let targetNode;
    if (newlyActiveAgent) {
        targetNode = newlyActiveAgent;
    } else {
        // Sort nodes to find the best one to follow
        const sortedNodes = [...candidateNodes].sort((a, b) => {
            // For agents, prioritize by started_at (most recent first)
            if (a.type === 'agent' && b.type === 'agent') {
                const aTime = new Date(a.started_at || 0).getTime();
                const bTime = new Date(b.started_at || 0).getTime();
                return bTime - aTime; // Most recent first
            }
            
            // For tasks/bugs, prioritize in_progress status
            if ((a.type === 'task' || a.type === 'bug') && (b.type === 'task' || b.type === 'bug')) {
                const aInProgress = a.status === 'in_progress' ? 0 : 1;
                const bInProgress = b.status === 'in_progress' ? 0 : 1;
                
                if (aInProgress !== bInProgress) {
                    return aInProgress - bInProgress;
                }
            }
            
            // Secondary sort by updated_at (most recent first)
            const aTime = new Date(a.updated_at || 0).getTime();
            const bTime = new Date(b.updated_at || 0).getTime();
            return bTime - aTime;
        });
        
        targetNode = sortedNodes[0];
    }
    
    if (!targetNode) {
        return;
    }
    
    // Check if we're switching to a different node
    const currentFollowingId = state.get('ui.followingNodeId');
    const isNewTarget = currentFollowingId !== targetNode.id;
    
    if (isNewTarget) {
        // New node to follow - use smooth animation to switch to it
        state.set('ui.followingNodeId', targetNode.id);
        console.log(`Auto-following ${targetNode.type}: ${targetNode.id}`);
        
        // Smooth animation when switching to a new target
        if (canvas && targetNode.x !== undefined && targetNode.y !== undefined) {
            panToNode(targetNode.x, targetNode.y, {
                canvas: canvas,
                duration: 800 // Smooth 0.8-second pan to new target
            });
        }
    } else {
        // Continuously center on the same target node's position
        // Use instant centering to track node movement in real-time
        if (canvas && targetNode.x !== undefined && targetNode.y !== undefined) {
            centerOn(targetNode.x, targetNode.y);
        }
    }
}

/**
 * Update camera to keep focused node centered
 */
function updateFocusedNode() {
    const focusedNodeId = state.get('ui.focusedNode');
    
    // No focus lock active
    if (!focusedNodeId) {
        return;
    }
    
    // Find the focused node in visible nodes
    const focusedNode = visibleNodes.find(node => node.id === focusedNodeId);
    
    if (!focusedNode) {
        // Node is no longer visible or doesn't exist, clear focus
        state.set('ui.focusedNode', null);
        console.log('Focused node no longer visible, clearing focus');
        return;
    }
    
    // Keep camera centered on the focused node
    if (canvas && focusedNode.x !== undefined && focusedNode.y !== undefined) {
        centerOn(focusedNode.x, focusedNode.y);
    }
}

/**
 * Handle page visibility changes to pause/resume goodbye linger timer
 */
function handleGoodbyeVisibilityChange() {
    const GOODBYE_LINGER_MS = 3000;
    
    if (document.hidden) {
        // Page hidden - pause goodbye timer if active
        const goodbyeStartTime = state.get('ui.goodbyeStartTime');
        if (goodbyeStartTime) {
            const elapsed = Date.now() - goodbyeStartTime;
            goodbyeRemainingTime = Math.max(0, GOODBYE_LINGER_MS - elapsed);
            goodbyePauseStartTime = Date.now();
            
            console.log(`[Renderer] Tab hidden - pausing goodbye linger (${goodbyeRemainingTime}ms remaining)`);
        }
    } else {
        // Page visible again - resume goodbye timer if needed
        if (goodbyePauseStartTime && goodbyeRemainingTime !== null && goodbyeRemainingTime > 0) {
            const pauseDuration = Date.now() - goodbyePauseStartTime;
            
            // Adjust goodbyeStartTime forward by pause duration
            const goodbyeStartTime = state.get('ui.goodbyeStartTime');
            if (goodbyeStartTime) {
                state.set('ui.goodbyeStartTime', goodbyeStartTime + pauseDuration);
                console.log(`[Renderer] Tab visible - resuming goodbye linger (adjusted start time by ${pauseDuration}ms)`);
            }
            
            goodbyePauseStartTime = null;
            goodbyeRemainingTime = null;
        }
    }
}


/**
 * Animation loop
 */
function animate() {
    if (!isAnimating) return;
    
    animationTime = performance.now();
    
    // Update canvas size if needed
    resizeCanvas();
    
    // Clean up fully-faded departing agents
    const now = performance.now();
    for (const [agentId, departureTime] of departingAgents.entries()) {
        if (now - departureTime >= AGENT_DEPARTURE_FADE_MS) {
            departingAgents.delete(agentId);
        }
    }
    
    // Clean up expired NEW badges (after 10 seconds)
    const newBadges = state.get('ui.newBadges') || new Map();
    let badgesChanged = false;
    for (const [entityId, badgeTime] of newBadges.entries()) {
        if (now - badgeTime >= NEW_BADGE_DURATION_MS) {
            newBadges.delete(entityId);
            badgesChanged = true;
        }
    }
    if (badgesChanged) {
        state.set('ui.newBadges', newBadges);
    }
    
    // Clean up completed selection animations
    for (const [nodeId, selectionTime] of selectionAnimations.entries()) {
        if (now - selectionTime >= SELECTION_ANIMATION_DURATION) {
            selectionAnimations.delete(nodeId);
        }
    }
    
    // Auto-follow logic
    updateAutoFollow();
    
    // Focus lock logic (takes priority over auto-follow)
    updateFocusedNode();
    
    // Apply physics simulation
    applyPhysics();
    
    // Render the graph
    render();
    
    animationFrameId = requestAnimationFrame(animate);
}

/**
 * Draw a badge showing the number of selected nodes
 * @param {number} count - Number of selected nodes
 */
function drawSelectionBadge(count) {
    // Badge position (top-right corner with padding)
    const padding = 20;
    const badgeWidth = 120;
    const badgeHeight = 40;
    const x = canvas.width - badgeWidth - padding;
    const y = padding;
    
    // Draw background
    ctx.fillStyle = 'rgba(106, 155, 220, 0.9)';
    ctx.strokeStyle = '#4a7bb8';
    ctx.lineWidth = 2;
    
    // Rounded rectangle
    const radius = 8;
    ctx.beginPath();
    ctx.moveTo(x + radius, y);
    ctx.lineTo(x + badgeWidth - radius, y);
    ctx.quadraticCurveTo(x + badgeWidth, y, x + badgeWidth, y + radius);
    ctx.lineTo(x + badgeWidth, y + badgeHeight - radius);
    ctx.quadraticCurveTo(x + badgeWidth, y + badgeHeight, x + badgeWidth - radius, y + badgeHeight);
    ctx.lineTo(x + radius, y + badgeHeight);
    ctx.quadraticCurveTo(x, y + badgeHeight, x, y + badgeHeight - radius);
    ctx.lineTo(x, y + radius);
    ctx.quadraticCurveTo(x, y, x + radius, y);
    ctx.closePath();
    ctx.fill();
    ctx.stroke();
    
    // Draw text
    ctx.fillStyle = '#ffffff';
    ctx.font = 'bold 14px sans-serif';
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.fillText(`${count} selected`, x + badgeWidth / 2, y + badgeHeight / 2);
}

/**
 * Draw box selection overlay
 * @param {Object} box - Box selection coordinates {x1, y1, x2, y2}
 */
function drawBoxSelection(box) {
    const { x1, y1, x2, y2 } = box;
    
    // Calculate rectangle bounds
    const minX = Math.min(x1, x2);
    const minY = Math.min(y1, y2);
    const width = Math.abs(x2 - x1);
    const height = Math.abs(y2 - y1);
    
    // Draw selection rectangle
    ctx.strokeStyle = 'rgba(106, 155, 220, 0.8)';
    ctx.lineWidth = 2;
    ctx.setLineDash([5, 5]); // Dashed line
    ctx.strokeRect(minX, minY, width, height);
    ctx.setLineDash([]); // Reset to solid line
    
    // Fill with semi-transparent color
    ctx.fillStyle = 'rgba(106, 155, 220, 0.15)';
    ctx.fillRect(minX, minY, width, height);
}

/**
 * Main render function
 */
function render() {
    if (!ctx || !canvas) return;
    
    const colors = getCSSColors();
    
    // Clear canvas
    ctx.fillStyle = colors.bgSecondary;
    ctx.fillRect(0, 0, canvas.width, canvas.height);
    
    // Filter visible nodes
    filterVisibleNodes();
    
    if (visibleNodes.length === 0) {
        // Show connection status if still connecting
        const connectionStatus = state.getConnectionStatus();
        if (connectionStatus === ConnectionStatus.CONNECTING || 
            connectionStatus === ConnectionStatus.RECONNECTING) {
            renderEmptyState('Connecting...');
        } else {
            renderEmptyState('No matching nodes');
        }
        return;
    }
    
    // Draw edges first (below nodes)
    const edgeFilters = state.get('ui.edgeTypeFilters') || {};
    for (const edge of graphEdges) {
        if (edgeFilters[edge.edge_type] === false) continue;
        
        const fromNode = graphNodes.find(n => n.id === edge.from);
        const toNode = graphNodes.find(n => n.id === edge.to);
        
        if (!fromNode || !toNode) continue;
        if (!visibleNodeIds.has(edge.from) || !visibleNodeIds.has(edge.to)) continue;
        
        drawEdge(fromNode, toNode, edge);
    }
    
    // Draw nodes
    for (const node of visibleNodes) {
        drawNode(node);
    }
    
    // Draw multi-selection badge if multiple nodes selected
    if (selectedNodes.length > 1) {
        drawSelectionBadge(selectedNodes.length);
    }
    
    // Draw box selection overlay if active
    const boxSelection = state.get('ui.boxSelection');
    if (boxSelection) {
        drawBoxSelection(boxSelection);
    }
}

/**
 * Render empty state message
 * @param {string} message - Message to display
 */
function renderEmptyState(message) {
    const colors = getCSSColors();
    
    ctx.fillStyle = colors.textSecondary;
    ctx.font = '16px sans-serif';
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.fillText(message, canvas.width / 2, canvas.height / 2);
}

/**
 * Draw a single node
 * @param {Object} node - Node to draw
 */
function drawNode(node) {
    const zoom = getZoom();
    
    // Transform to screen coordinates
    const screenPos = worldToScreen(node.x, node.y, canvas);
    const radius = node.radius * zoom;
    
    // Viewport culling: skip nodes outside visible area
    const margin = 100; // px margin to avoid pop-in
    if (screenPos.x + radius < -margin ||
        screenPos.x - radius > canvas.width + margin ||
        screenPos.y + radius < -margin ||
        screenPos.y - radius > canvas.height + margin) {
        return; // Node is off-screen, skip drawing
    }
    
    const isHovered = node === hoveredNode;
    const isDragging = node === draggedNode;
    const isSelected = selectedNodes.includes(node.id);
    const isMultiSelect = selectedNodes.length > 1;
    
    // Get node color
    const color = getNodeColor(node);
    
    // Calculate opacity (for dimmed/fading nodes)
    let opacity = 1.0;
    
    // Check for reveal animation opacity (fade-in)
    const revealOpacity = getRevealOpacity(node.id);
    if (revealOpacity !== null) {
        opacity = revealOpacity;
    }
    
    // Check for collapse animation opacity (fade-out)
    const fadeOutOpacity = getFadeOutOpacity(node.id);
    if (fadeOutOpacity !== null) {
        opacity = Math.min(opacity, fadeOutOpacity);
    }
    
    // Check for departing agent fade
    if (node._departing && node.type === 'agent') {
        const departureTime = departingAgents.get(node.id);
        if (departureTime) {
            const elapsed = performance.now() - departureTime;
            const progress = Math.min(elapsed / AGENT_DEPARTURE_FADE_MS, 1.0);
            opacity = Math.min(opacity, 1.0 - progress);
        }
    }
    ctx.globalAlpha = opacity;
    
    // Draw selection highlight
    if (isSelected) {
        // Calculate animation progress for newly selected nodes
        let animScale = 1.0;
        let animAlpha = 1.0;
        const selectionTime = selectionAnimations.get(node.id);
        if (selectionTime) {
            const elapsed = performance.now() - selectionTime;
            if (elapsed < SELECTION_ANIMATION_DURATION) {
                const progress = elapsed / SELECTION_ANIMATION_DURATION;
                // Pulse effect: scale slightly larger then back to normal
                animScale = 1.0 + 0.15 * Math.sin(progress * Math.PI);
                // Glow effect: alpha pulsates
                animAlpha = 0.7 + 0.3 * (1 - progress);
            } else {
                // Animation complete, remove from tracking
                selectionAnimations.delete(node.id);
            }
        }
        
        ctx.save();
        ctx.globalAlpha *= animAlpha;
        
        ctx.beginPath();
        const highlightRadius = (radius + 10 * zoom) * animScale;
        drawNodeShapePath(ctx, node.type, screenPos.x, screenPos.y, highlightRadius);
        
        // Different style for multi-selection vs single selection
        if (isMultiSelect) {
            ctx.strokeStyle = '#6a9bdc'; // Blue for multi-select
            ctx.lineWidth = 3 * animScale;
            ctx.stroke();
            ctx.fillStyle = 'rgba(106, 155, 220, 0.2)';
            ctx.fill();
        } else {
            ctx.strokeStyle = '#f0ad4e'; // Orange for single select
            ctx.lineWidth = 4 * animScale;
            ctx.stroke();
            ctx.fillStyle = 'rgba(240, 173, 78, 0.15)';
            ctx.fill();
        }
        
        ctx.restore();
    }
    
    // Draw programmatic highlight (e.g., from entity link hover)
    if (highlightedNodeId === node.id && highlightStartTime) {
        const elapsed = performance.now() - highlightStartTime;
        if (elapsed < HIGHLIGHT_ANIMATION_DURATION) {
            const progress = elapsed / HIGHLIGHT_ANIMATION_DURATION;
            // Pulsing glow effect
            const pulseFreq = 3; // Number of pulses over duration
            const pulseValue = Math.sin(progress * Math.PI * pulseFreq);
            const scale = 1.0 + 0.2 * pulseValue;
            const alpha = 0.8 * (1 - progress * 0.5); // Fade out slowly
            
            ctx.save();
            ctx.globalAlpha *= alpha;
            
            ctx.beginPath();
            const highlightRadius = (radius + 12 * zoom) * scale;
            drawNodeShapePath(ctx, node.type, screenPos.x, screenPos.y, highlightRadius);
            
            // Bright cyan/magenta highlight to stand out from selection
            ctx.strokeStyle = '#ff00ff'; // Magenta
            ctx.lineWidth = 5 * scale;
            ctx.stroke();
            ctx.fillStyle = 'rgba(255, 0, 255, 0.25)';
            ctx.fill();
            
            ctx.restore();
        } else {
            // Animation complete, clear highlight
            highlightedNodeId = null;
            highlightStartTime = null;
        }
    }
    
    // Draw drag/hover highlight
    if (isDragging || isHovered) {
        ctx.beginPath();
        drawNodeShapePath(ctx, node.type, screenPos.x, screenPos.y, radius + 8 * zoom);
        ctx.fillStyle = isDragging ? 'rgba(74, 144, 226, 0.3)' : 'rgba(74, 144, 226, 0.2)';
        ctx.fill();
        ctx.strokeStyle = isDragging ? '#4a90e2' : '#6aa8f0';
        ctx.lineWidth = 3;
        ctx.stroke();
    }
    
    // Draw queued indicator (teal glow)
    if (isNodeQueued(node.id) && node.type !== 'queue' && node.type !== 'agent' && node.type !== 'doc') {
        ctx.beginPath();
        drawNodeShapePath(ctx, node.type, screenPos.x, screenPos.y, radius + 6 * zoom);
        ctx.strokeStyle = '#20b2aa';
        ctx.lineWidth = 3;
        ctx.stroke();
        
        ctx.beginPath();
        drawNodeShapePath(ctx, node.type, screenPos.x, screenPos.y, radius + 4 * zoom);
        ctx.fillStyle = 'rgba(32, 178, 170, 0.15)';
        ctx.fill();
    }
    
    // Draw dotted yellow border for task nodes (not in_progress)
    if (node.type === 'task' && node.status !== 'in_progress') {
        ctx.beginPath();
        ctx.arc(screenPos.x, screenPos.y, radius + 4 * zoom, 0, Math.PI * 2);
        ctx.strokeStyle = 'rgba(255, 215, 0, 0.7)';
        ctx.lineWidth = 2 * zoom;
        ctx.setLineDash([4 * zoom, 4 * zoom]);
        ctx.stroke();
        ctx.setLineDash([]);
    }
    
    // Draw animated spiral for active agents
    if (node.type === 'agent' && node.status === 'active') {
        drawAgentSpiral(screenPos.x, screenPos.y, radius);
    }
    
    // Draw animated rings for in_progress tasks/bugs/ideas with active agents
    if (node.status === 'in_progress' && node.type !== 'queue' && node.type !== 'agent' && node.type !== 'doc' && hasActiveAgent(node.id)) {
        drawInProgressRings(screenPos.x, screenPos.y, radius);
    }
    
    // Draw main node shape
    ctx.beginPath();
    drawNodeShapePath(ctx, node.type, screenPos.x, screenPos.y, radius);
    ctx.fillStyle = color;
    ctx.fill();
    ctx.strokeStyle = (isHovered || isDragging) ? '#ffffff' : '#e8edf3';
    ctx.lineWidth = (isHovered || isDragging) ? 3 : 2;
    ctx.stroke();
    
    // Draw node text (skip for agents)
    if (node.type !== 'agent') {
        drawNodeText(node, screenPos, radius);
    } else {
        drawAgentLabel(node, screenPos, radius);
    }
    
    // Draw PRD label for PRD doc nodes
    drawPRDLabel(node, screenPos, radius);
    
    // Draw node type capsule labels
    drawNodeTypeCapsule(node, screenPos, radius);
    
    // Draw NEW badge if this node triggered an event recently
    drawNewBadge(node, screenPos, radius);
    
    ctx.globalAlpha = 1.0;
}

/**
 * Draw animated spiral for active agent nodes
 */
function drawAgentSpiral(cx, cy, radius) {
    const zoom = getZoom();
    const rotationSpeed = 0.002;
    const spiralRadius = radius * 0.6;
    
    ctx.save();
    ctx.translate(cx, cy);
    ctx.rotate(animationTime * rotationSpeed);
    
    const armCount = 3;
    const armWidth = 3 * zoom;
    
    for (let arm = 0; arm < armCount; arm++) {
        const baseAngle = (arm * Math.PI * 2) / armCount;
        
        const gradient = ctx.createLinearGradient(
            Math.cos(baseAngle) * spiralRadius * 0.3,
            Math.sin(baseAngle) * spiralRadius * 0.3,
            Math.cos(baseAngle) * spiralRadius,
            Math.sin(baseAngle) * spiralRadius
        );
        gradient.addColorStop(0, '#1e3a5f');
        gradient.addColorStop(1, '#d0d0d0');
        
        ctx.beginPath();
        ctx.strokeStyle = gradient;
        ctx.lineWidth = armWidth;
        ctx.lineCap = 'round';
        
        const startR = spiralRadius * 0.2;
        const endR = spiralRadius;
        const curveAngle = Math.PI * 0.5;
        
        ctx.moveTo(
            Math.cos(baseAngle) * startR,
            Math.sin(baseAngle) * startR
        );
        ctx.quadraticCurveTo(
            Math.cos(baseAngle + curveAngle * 0.5) * spiralRadius * 0.7,
            Math.sin(baseAngle + curveAngle * 0.5) * spiralRadius * 0.7,
            Math.cos(baseAngle + curveAngle) * endR,
            Math.sin(baseAngle + curveAngle) * endR
        );
        ctx.stroke();
    }
    
    ctx.restore();
}

/**
 * Draw counter-rotating hatched rings for in_progress tasks
 */
function drawInProgressRings(cx, cy, radius) {
    const zoom = getZoom();
    const rotationSpeed = 0.001;
    const outerRingRadius = radius + 14 * zoom;
    const innerRingRadius = radius + 8 * zoom;
    const ringWidth = 2.5 * zoom;
    const hatchCount = 12;
    const hatchLength = Math.PI / 18;
    
    // Outer ring - rotates clockwise
    ctx.save();
    ctx.translate(cx, cy);
    ctx.rotate(animationTime * rotationSpeed);
    ctx.strokeStyle = 'rgba(240, 173, 78, 0.8)';
    ctx.lineWidth = ringWidth;
    ctx.lineCap = 'round';
    for (let i = 0; i < hatchCount; i++) {
        const startAngle = (i * Math.PI * 2) / hatchCount;
        ctx.beginPath();
        ctx.arc(0, 0, outerRingRadius, startAngle, startAngle + hatchLength);
        ctx.stroke();
    }
    ctx.restore();
    
    // Inner ring - rotates counter-clockwise
    ctx.save();
    ctx.translate(cx, cy);
    ctx.rotate(-animationTime * rotationSpeed * 1.5);
    ctx.strokeStyle = 'rgba(255, 200, 100, 0.6)';
    ctx.lineWidth = ringWidth * 0.8;
    ctx.lineCap = 'round';
    for (let i = 0; i < hatchCount; i++) {
        const startAngle = (i * Math.PI * 2) / hatchCount + Math.PI / hatchCount;
        ctx.beginPath();
        ctx.arc(0, 0, innerRingRadius, startAngle, startAngle + hatchLength * 0.8);
        ctx.stroke();
    }
    ctx.restore();
}

/**
 * Draw node text (short_name and ID)
 */
function drawNodeText(node, screenPos, _radius) {
    const zoom = getZoom();
    const isHovered = node === hoveredNode;
    const isDragging = node === draggedNode;
    
    ctx.fillStyle = '#1a2332';
    const baseFontSize = 12 * Math.max(0.8, Math.min(1.5, zoom));
    const smallFontSize = baseFontSize * 0.75;
    ctx.textAlign = 'center';
    
    if (node.short_name) {
        const labelLines = formatNodeLabel(node.short_name, 8, 2);
        const lineHeight = baseFontSize * 1.2;
        
        const totalLines = labelLines.length + 1;
        const totalHeight = (totalLines - 1) * lineHeight;
        const startY = screenPos.y - totalHeight / 2 + baseFontSize / 2;
        
        ctx.font = (isHovered || isDragging) ? `bold ${baseFontSize}px sans-serif` : `${baseFontSize}px sans-serif`;
        ctx.textBaseline = 'middle';
        labelLines.forEach((line, i) => {
            ctx.fillText(line, screenPos.x, startY + i * lineHeight);
        });
        
        ctx.font = `${smallFontSize}px sans-serif`;
        ctx.fillStyle = 'rgba(26, 35, 50, 0.7)';
        ctx.fillText(node.id, screenPos.x, startY + labelLines.length * lineHeight);
    } else {
        ctx.font = (isHovered || isDragging) ? `bold ${baseFontSize}px sans-serif` : `${baseFontSize}px sans-serif`;
        ctx.textBaseline = 'middle';
        ctx.fillText(node.id, screenPos.x, screenPos.y);
    }
}

/**
 * Draw agent label above the node
 */
function drawAgentLabel(node, screenPos, radius) {
    const zoom = getZoom();
    const agent = node._agent;
    if (!agent) return;
    
    const baseFontSize = 11 * Math.max(0.7, Math.min(1.3, zoom));
    const pillPadding = 4 * zoom;
    const pillHeight = baseFontSize + pillPadding * 2;
    const pillY = screenPos.y - radius - pillHeight - 6 * zoom;
    
    const agentName = agent.name || node.id;
    const action = agent.current_action || (agent.status === 'active' ? 'working' : agent.status);
    const displayText = `${agentName}: ${action}`;
    
    ctx.font = `${baseFontSize}px sans-serif`;
    const textWidth = ctx.measureText(displayText).width;
    const pillWidth = textWidth + pillPadding * 4;
    
    // Calculate fade opacity for goodbye with easing
    let fadeAlpha = 1.0;
    if (agent.goodbye_at) {
        const goodbyeTime = new Date(agent.goodbye_at).getTime();
        const elapsed = Date.now() - goodbyeTime;
        const fadeDuration = 5000;
        const progress = Math.min(elapsed / fadeDuration, 1.0);
        // Use easeInQuad for smooth acceleration into fade
        fadeAlpha = Math.max(0, 1 - easeInQuad(progress));
    }
    
    ctx.save();
    ctx.globalAlpha = fadeAlpha * 0.9;
    
    const isGoodbye = agent.current_action === 'goodbye';
    const bgColor = isGoodbye ? 'rgba(180, 80, 80, 0.95)' : 'rgba(30, 58, 95, 0.95)';
    
    ctx.fillStyle = bgColor;
    ctx.beginPath();
    const pillX = screenPos.x - pillWidth / 2;
    const pillRadius = pillHeight / 2;
    ctx.roundRect(pillX, pillY, pillWidth, pillHeight, pillRadius);
    ctx.fill();
    
    ctx.globalAlpha = fadeAlpha;
    ctx.fillStyle = '#ffffff';
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.fillText(displayText, screenPos.x, pillY + pillHeight / 2);
    
    ctx.restore();
}

/**
 * Draw PRD label above doc nodes that are PRDs.
 * Shows "PRD" badge on top and a brief summary (short_name or truncated title) below.
 */
function drawPRDLabel(node, screenPos, radius) {
    if (node.type !== 'doc' || node.doc_type !== 'prd') return;
    
    const zoom = getZoom();
    const baseFontSize = 13 * Math.max(0.7, Math.min(1.3, zoom));
    const summaryFontSize = 11 * Math.max(0.7, Math.min(1.3, zoom));
    const pillPadding = 6 * zoom;
    const lineGap = 4 * zoom;
    
    // Get summary text: prefer short_name, fall back to truncated title
    const summaryText = node.short_name || (node.title ? truncateText(node.title, 30) : '');
    const hasSummary = summaryText.length > 0;
    
    // Calculate dimensions
    const prdText = 'PRD';
    ctx.font = `bold ${baseFontSize}px sans-serif`;
    const prdTextWidth = ctx.measureText(prdText).width;
    
    let summaryTextWidth = 0;
    if (hasSummary) {
        ctx.font = `${summaryFontSize}px sans-serif`;
        summaryTextWidth = ctx.measureText(summaryText).width;
    }
    
    // Pill width accommodates both lines with padding
    const maxTextWidth = Math.max(prdTextWidth, summaryTextWidth);
    const pillWidth = maxTextWidth + pillPadding * 4;
    
    // Pill height depends on whether we have a summary
    const prdLineHeight = baseFontSize + pillPadding * 2;
    const summaryLineHeight = hasSummary ? summaryFontSize + pillPadding : 0;
    const pillHeight = prdLineHeight + summaryLineHeight + (hasSummary ? lineGap : 0);
    
    const pillY = screenPos.y - radius - pillHeight - 8 * zoom;
    const pillX = screenPos.x - pillWidth / 2;
    const pillRadius = Math.min(pillHeight / 4, 8 * zoom);
    
    ctx.save();
    
    // Draw pill background
    ctx.fillStyle = 'rgba(147, 51, 234, 0.95)';  // Purple background
    ctx.beginPath();
    ctx.roundRect(pillX, pillY, pillWidth, pillHeight, pillRadius);
    ctx.fill();
    
    // Draw "PRD" text (top)
    ctx.fillStyle = '#ffffff';
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.font = `bold ${baseFontSize}px sans-serif`;
    const prdY = pillY + pillPadding + baseFontSize / 2;
    ctx.fillText(prdText, screenPos.x, prdY);
    
    // Draw summary text (bottom) if available
    if (hasSummary) {
        ctx.font = `${summaryFontSize}px sans-serif`;
        ctx.fillStyle = 'rgba(255, 255, 255, 0.85)';
        const summaryY = prdY + baseFontSize / 2 + lineGap + summaryFontSize / 2 + pillPadding / 2;
        ctx.fillText(summaryText, screenPos.x, summaryY);
    }
    
    ctx.restore();
}

/**
 * Truncate text to a maximum length with ellipsis
 */
function truncateText(text, maxLength) {
    if (!text || text.length <= maxLength) return text;
    return text.substring(0, maxLength - 1) + '…';
}

/**
 * Draw node type capsule label above nodes (except PRD which has its own)
 */
function drawNodeTypeCapsule(node, screenPos, radius) {
    // Skip PRD docs (they have their own label) and tests
    if (node.type === 'test') return;
    if (node.type === 'doc' && node.doc_type === 'prd') return;
    
    // Define capsule text and color for each node type
    let displayText = '';
    let backgroundColor = '';
    
    switch (node.type) {
        case 'agent':
            displayText = 'Agent Worker';
            backgroundColor = 'rgba(0, 212, 255, 0.95)'; // Cyan
            break;
        case 'idea':
            displayText = 'Idea';
            backgroundColor = 'rgba(255, 255, 255, 0.95)'; // White for ideas
            break;
        case 'doc':
            displayText = 'Doc';
            // Use doc type color or default blue
            if (node.doc_type === 'note') {
                backgroundColor = 'rgba(232, 184, 74, 0.95)'; // Yellow
            } else if (node.doc_type === 'handoff') {
                backgroundColor = 'rgba(232, 125, 74, 0.95)'; // Orange
            } else {
                backgroundColor = 'rgba(74, 144, 226, 0.95)'; // Blue
            }
            break;
        case 'milestone':
            displayText = 'Milestone';
            backgroundColor = 'rgba(255, 140, 0, 0.95)'; // Orange
            break;
        case 'bug':
            displayText = 'Bug';
            backgroundColor = 'rgba(224, 120, 120, 0.95)'; // Red
            break;
        case 'task':
            displayText = 'Task';
            backgroundColor = 'rgba(91, 192, 222, 0.95)'; // Blue
            break;
        case 'queue':
            displayText = 'Queue';
            backgroundColor = 'rgba(32, 178, 170, 0.95)'; // Teal
            break;
        default:
            return; // Don't draw capsule for unknown types
    }
    
    const zoom = getZoom();
    const baseFontSize = 13 * Math.max(0.7, Math.min(1.3, zoom));
    const pillPadding = 6 * zoom;
    const pillHeight = baseFontSize + pillPadding * 2;
    const pillY = screenPos.y - radius - pillHeight - 8 * zoom;
    
    ctx.font = `bold ${baseFontSize}px sans-serif`;
    const textWidth = ctx.measureText(displayText).width;
    const pillWidth = textWidth + pillPadding * 4;
    
    ctx.save();
    
    // Draw pill background
    ctx.fillStyle = backgroundColor;
    ctx.beginPath();
    const pillX = screenPos.x - pillWidth / 2;
    const pillRadius = pillHeight / 2;
    ctx.roundRect(pillX, pillY, pillWidth, pillHeight, pillRadius);
    ctx.fill();
    
    // Draw text - use dark text for white ideas, white for others
    ctx.fillStyle = node.type === 'idea' ? '#1a2332' : '#ffffff';
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.fillText(displayText, screenPos.x, pillY + pillHeight / 2);
    
    ctx.restore();
}

/**
 * Format node label with word wrapping
 */
function formatNodeLabel(text, maxCharsPerLine, maxLines) {
    if (!text) return [];
    
    const words = text.split(/\s+/);
    const lines = [];
    let currentLine = '';
    
    for (const word of words) {
        const testLine = currentLine ? `${currentLine} ${word}` : word;
        if (testLine.length <= maxCharsPerLine) {
            currentLine = testLine;
        } else {
            if (currentLine) {
                lines.push(currentLine);
                if (lines.length >= maxLines) break;
            }
            currentLine = word.length > maxCharsPerLine ? word.substring(0, maxCharsPerLine) : word;
        }
    }
    
    if (currentLine && lines.length < maxLines) {
        lines.push(currentLine);
    }
    
    return lines;
}

/**
 * Check if a node is in the queue
 */
function isNodeQueued(nodeId) {
    const edges = state.get('edges') || [];
    return edges.some(edge => 
        edge.source === nodeId && 
        edge.edge_type === 'queued'
    );
}

/**
 * Check if a node has an agent actively working on it
 */
function hasActiveAgent(nodeId) {
    const edges = state.get('edges') || [];
    return edges.some(edge => 
        edge.target === nodeId && 
        edge.edge_type === 'working_on'
    );
}

/**
 * Draw an edge between two nodes
 */
function drawEdge(fromNode, toNode, edge) {
    const zoom = getZoom();
    const style = getEdgeStyle(edge.edge_type);
    
    // Calculate angle and distance
    const dx = toNode.x - fromNode.x;
    const dy = toNode.y - fromNode.y;
    const angle = Math.atan2(dy, dx);
    const distance = Math.sqrt(dx * dx + dy * dy);
    
    // Skip if nodes are overlapping
    if (distance < fromNode.radius + toNode.radius) return;
    
    // Calculate start/end points at edge of nodes
    const startX = fromNode.x + Math.cos(angle) * fromNode.radius;
    const startY = fromNode.y + Math.sin(angle) * fromNode.radius;
    const endX = toNode.x - Math.cos(angle) * toNode.radius;
    const endY = toNode.y - Math.sin(angle) * toNode.radius;
    
    // Transform to screen coordinates
    const p1 = worldToScreen(startX, startY, canvas);
    const p2 = worldToScreen(endX, endY, canvas);
    
    // Apply style
    ctx.strokeStyle = style.color;
    ctx.fillStyle = style.color;
    ctx.lineWidth = style.lineWidth * zoom;
    
    // Set dash pattern
    if (style.dashed) {
        ctx.setLineDash([6, 4]);
        if (style.animated) {
            const speed = 0.03;
            ctx.lineDashOffset = -(animationTime * speed) % 10;
        } else {
            ctx.lineDashOffset = 0;
        }
    } else {
        ctx.setLineDash([]);
        ctx.lineDashOffset = 0;
    }
    
    // Draw line
    ctx.beginPath();
    ctx.moveTo(p1.x, p1.y);
    ctx.lineTo(p2.x, p2.y);
    ctx.stroke();
    
    // Reset dash for arrow heads
    ctx.setLineDash([]);
    
    // Calculate midpoint for arrow
    const midX = (p1.x + p2.x) / 2;
    const midY = (p1.y + p2.y) / 2;
    const headLength = 10 * zoom;
    const screenAngle = Math.atan2(p2.y - p1.y, p2.x - p1.x);
    
    // Draw arrow head at midpoint
    ctx.beginPath();
    ctx.moveTo(midX, midY);
    ctx.lineTo(
        midX - headLength * Math.cos(screenAngle - Math.PI / 6),
        midY - headLength * Math.sin(screenAngle - Math.PI / 6)
    );
    ctx.lineTo(
        midX - headLength * Math.cos(screenAngle + Math.PI / 6),
        midY - headLength * Math.sin(screenAngle + Math.PI / 6)
    );
    ctx.closePath();
    ctx.fill();
    
    // Draw reverse arrow for bidirectional edges
    if (edge.bidirectional) {
        const reverseAngle = screenAngle + Math.PI;
        ctx.beginPath();
        ctx.moveTo(midX, midY);
        ctx.lineTo(
            midX - headLength * Math.cos(reverseAngle - Math.PI / 6),
            midY - headLength * Math.sin(reverseAngle - Math.PI / 6)
        );
        ctx.lineTo(
            midX - headLength * Math.cos(reverseAngle + Math.PI / 6),
            midY - headLength * Math.sin(reverseAngle + Math.PI / 6)
        );
        ctx.closePath();
        ctx.fill();
    }
}

/**
 * Draw NEW badge for nodes that triggered events
 * @param {Object} node - Node to draw badge for
 * @param {Object} screenPos - Screen position {x, y}
 * @param {number} radius - Node radius
 */
function drawNewBadge(node, screenPos, radius) {
    const newBadges = state.get('ui.newBadges') || new Map();
    const badgeTime = newBadges.get(node.id);
    
    if (!badgeTime) {
        return; // No badge for this node
    }
    
    const zoom = getZoom();
    const now = performance.now();
    const elapsed = now - badgeTime;
    
    // Calculate fade opacity with smooth fade-in and fade-out
    let opacity = 1.0;
    
    // Fade-in animation (first 300ms)
    if (elapsed < NEW_BADGE_FADE_IN_MS) {
        const fadeInProgress = elapsed / NEW_BADGE_FADE_IN_MS;
        opacity = easeOutQuad(fadeInProgress); // Smooth fade-in
    }
    // Fade-out animation (last 2 seconds)
    else {
        const fadeStart = NEW_BADGE_DURATION_MS - NEW_BADGE_FADE_OUT_MS;
        if (elapsed > fadeStart) {
            const fadeProgress = (elapsed - fadeStart) / NEW_BADGE_FADE_OUT_MS;
            opacity = 1.0 - easeInQuad(fadeProgress); // Smooth fade-out
        }
    }
    
    if (opacity <= 0) {
        return; // Fully faded
    }
    
    // Badge position: top-right of node
    const badgePadding = 8 * zoom;
    const badgeX = screenPos.x + radius + badgePadding;
    const badgeY = screenPos.y - radius;
    
    // Badge dimensions
    const badgeHeight = 20 * zoom;
    const badgePaddingX = 8 * zoom;
    const fontSize = 11 * zoom;
    
    // Measure text
    ctx.font = `bold ${fontSize}px sans-serif`;
    const text = 'NEW';
    const textWidth = ctx.measureText(text).width;
    const badgeWidth = textWidth + badgePaddingX * 2;
    
    // Draw badge background
    ctx.save();
    ctx.globalAlpha *= opacity;
    
    const cornerRadius = 4 * zoom;
    ctx.beginPath();
    ctx.moveTo(badgeX + cornerRadius, badgeY);
    ctx.lineTo(badgeX + badgeWidth - cornerRadius, badgeY);
    ctx.quadraticCurveTo(badgeX + badgeWidth, badgeY, badgeX + badgeWidth, badgeY + cornerRadius);
    ctx.lineTo(badgeX + badgeWidth, badgeY + badgeHeight - cornerRadius);
    ctx.quadraticCurveTo(badgeX + badgeWidth, badgeY + badgeHeight, badgeX + badgeWidth - cornerRadius, badgeY + badgeHeight);
    ctx.lineTo(badgeX + cornerRadius, badgeY + badgeHeight);
    ctx.quadraticCurveTo(badgeX, badgeY + badgeHeight, badgeX, badgeY + badgeHeight - cornerRadius);
    ctx.lineTo(badgeX, badgeY + cornerRadius);
    ctx.quadraticCurveTo(badgeX, badgeY, badgeX + cornerRadius, badgeY);
    ctx.closePath();
    
    // Gradient background (bright accent)
    const gradient = ctx.createLinearGradient(badgeX, badgeY, badgeX, badgeY + badgeHeight);
    gradient.addColorStop(0, '#ff6b6b'); // Bright red
    gradient.addColorStop(1, '#ff4757'); // Darker red
    ctx.fillStyle = gradient;
    ctx.fill();
    
    // Border
    ctx.strokeStyle = '#ff3838';
    ctx.lineWidth = 1.5 * zoom;
    ctx.stroke();
    
    // Draw text
    ctx.fillStyle = '#ffffff';
    ctx.font = `bold ${fontSize}px sans-serif`;
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.fillText(text, badgeX + badgeWidth / 2, badgeY + badgeHeight / 2);
    
    ctx.restore();
}

// ============================================
// Public API for interaction handling
// ============================================

/**
 * Set the hovered node
 * @param {Object|null} node - Node being hovered, or null
 */
export function setHoveredNode(node) {
    if (hoveredNode !== node) {
        hoveredNode = node;
        // Update state for overlay panel
        state.set('ui.hoveredNode', node ? node.id : null);
        scheduleRender();
    }
}

/**
 * Set the dragged node
 * @param {Object|null} node - Node being dragged, or null
 */
export function setDraggedNode(node) {
    if (draggedNode !== node) {
        draggedNode = node;
        scheduleRender();
    }
}

/**
 * Find node at screen position
 * @param {number} screenX - Screen X coordinate
 * @param {number} screenY - Screen Y coordinate
 * @returns {Object|null} Node at position, or null
 */
export function findNodeAtPosition(screenX, screenY) {
    const worldPos = screenToWorld(screenX, screenY, canvas);
    
    // Search in reverse order (topmost first)
    for (let i = visibleNodes.length - 1; i >= 0; i--) {
        const node = visibleNodes[i];
        const dx = worldPos.x - node.x;
        const dy = worldPos.y - node.y;
        const dist = Math.sqrt(dx * dx + dy * dy);
        
        if (dist <= node.radius) {
            return node;
        }
    }
    
    return null;
}

/**
 * Calculate distance from a point to an edge (line segment)
 * @param {number} px - Point X (world coordinates)
 * @param {number} py - Point Y (world coordinates)
 * @param {Object} edge - Edge object with from/to
 * @returns {number} Distance to edge, or Infinity if invalid
 */
function distanceToEdge(px, py, edge) {
    const fromNode = graphNodes.find(n => n.id === edge.from);
    const toNode = graphNodes.find(n => n.id === edge.to);
    if (!fromNode || !toNode) return Infinity;
    
    // Calculate edge endpoints at node boundaries
    const dx = toNode.x - fromNode.x;
    const dy = toNode.y - fromNode.y;
    const dist = Math.sqrt(dx * dx + dy * dy);
    if (dist < fromNode.radius + toNode.radius) return Infinity;
    
    const angle = Math.atan2(dy, dx);
    const x1 = fromNode.x + Math.cos(angle) * fromNode.radius;
    const y1 = fromNode.y + Math.sin(angle) * fromNode.radius;
    const x2 = toNode.x - Math.cos(angle) * toNode.radius;
    const y2 = toNode.y - Math.sin(angle) * toNode.radius;
    
    // Calculate distance from point to line segment
    const lineDx = x2 - x1;
    const lineDy = y2 - y1;
    const lineLen = Math.sqrt(lineDx * lineDx + lineDy * lineDy);
    if (lineLen === 0) return Infinity;
    
    // Project point onto line, clamped to segment
    const t = Math.max(0, Math.min(1, ((px - x1) * lineDx + (py - y1) * lineDy) / (lineLen * lineLen)));
    const projX = x1 + t * lineDx;
    const projY = y1 + t * lineDy;
    
    return Math.sqrt((px - projX) * (px - projX) + (py - projY) * (py - projY));
}

/**
 * Find edge at screen position
 * @param {number} screenX - Screen X coordinate
 * @param {number} screenY - Screen Y coordinate
 * @param {number} threshold - Distance threshold in pixels (default 8)
 * @returns {Object|null} Edge at position, or null
 */
export function findEdgeAtPosition(screenX, screenY, threshold = 8) {
    const worldPos = screenToWorld(screenX, screenY, canvas);
    const zoom = getZoom();
    
    // Adjust threshold for zoom level
    const adjustedThreshold = threshold / zoom;
    
    let closestEdge = null;
    let closestDistance = adjustedThreshold;
    
    const edgeFilters = state.get('ui.edgeTypeFilters') || {};
    
    for (const edge of graphEdges) {
        // Skip edges that are filtered out
        if (edgeFilters[edge.edge_type] === false) continue;
        
        // Skip edges where either endpoint is hidden
        if (!visibleNodeIds.has(edge.from) || !visibleNodeIds.has(edge.to)) {
            continue;
        }
        
        const distance = distanceToEdge(worldPos.x, worldPos.y, edge);
        if (distance < closestDistance) {
            closestDistance = distance;
            closestEdge = edge;
        }
    }
    
    return closestEdge;
}

/**
 * Move a node to a new position
 * @param {Object} node - Node to move
 * @param {number} worldX - New world X coordinate
 * @param {number} worldY - New world Y coordinate
 */
export function moveNode(node, worldX, worldY) {
    node.x = worldX;
    node.y = worldY;
    node.vx = 0;
    node.vy = 0;
    scheduleRender();
}

/**
 * Get current graph nodes
 * @returns {Array} Array of graph nodes
 */
export function getNodes() {
    return graphNodes;
}

/**
 * Get visible nodes
 * @returns {Array} Array of visible nodes
 */
export function getVisibleNodes() {
    return visibleNodes;
}

/**
 * Get the canvas element
 * @returns {HTMLCanvasElement} Canvas element
 */
export function getCanvas() {
    return canvas;
}

/**
 * Mark an agent as departing (triggers fade animation)
 * @param {string} agentId - Agent ID
 */
export function markAgentDeparting(agentId) {
    departingAgents.set(agentId, performance.now());
    startAnimation();
}

/**
 * Highlight a node with a pulsing glow animation
 * @param {string} nodeId - Node ID to highlight
 */
export function highlightNode(nodeId) {
    if (!nodeId) return;
    
    highlightedNodeId = nodeId;
    highlightStartTime = performance.now();
    startAnimation(); // Ensure animation loop is running
    
    // Optionally pan to the node if it's not in view
    const node = graphNodes.find(n => n.id === nodeId);
    if (node) {
        panToNode(node);
    }
}

/**
 * Clear the current node highlight
 */
export function clearHighlight() {
    highlightedNodeId = null;
    highlightStartTime = null;
    scheduleRender();
}

// Export for external use
export { animationTime };
