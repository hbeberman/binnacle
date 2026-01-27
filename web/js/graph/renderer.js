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

// Animation constants
const AGENT_DEPARTURE_FADE_MS = 5000;
const STABLE_THRESHOLD = 0.01;
const STABLE_FRAMES_REQUIRED = 60;

// Renderer state
let canvas = null;
let ctx = null;
let animationFrameId = null;
let isAnimating = false;
let animationTime = 0;
let stableFrameCount = 0;

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

/**
 * Initialize the graph renderer with a canvas element
 * @param {HTMLCanvasElement} canvasElement - The canvas to render to
 * @param {Object} callbacks - Optional callback functions
 * @param {Function} callbacks.onNodeDoubleClick - Called when double-clicking a node
 */
export function init(canvasElement, callbacks = {}) {
    canvas = canvasElement;
    ctx = canvas.getContext('2d');
    
    // Set initial canvas size
    resizeCanvas();
    
    // Initialize camera controls (panning, zooming, node dragging, hover)
    camera.init(canvasElement, {
        onNodeDoubleClick: callbacks.onNodeDoubleClick || null
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
 * Build graph nodes from entities
 */
function buildGraphNodes() {
    const existingNodes = new Map(graphNodes.map(n => [n.id, n]));
    
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
    
    graphNodes = allEntities.map((entity, index) => {
        const existing = existingNodes.get(entity.id);
        
        // For agent nodes, store the agent data in _agent for label rendering
        const agentData = entity.type === 'agent' ? entity : entity._agent;
        
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
            // New node - place in circular layout
            const totalNodes = allEntities.length;
            const angle = (index / totalNodes) * 2 * Math.PI;
            const radius = 300;  // Initial radius in world units
            
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
                x: Math.cos(angle) * radius,
                y: Math.sin(angle) * radius,
                vx: 0,
                vy: 0,
                radius: 30  // Node radius in world units
            };
        }
    });
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
    
    visibleNodes = graphNodes.filter(node => {
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
function onSelectionChanged(_nodeId) {
    // Keep for backward compatibility with components that use ui.selectedNode
    scheduleRender();
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
    stableFrameCount = 0;
    
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
        const deviation = distance - physics.springRestingLength;
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
    
    // Don't auto-follow if disabled or user has paused it
    if (!autoFollow || userPaused) {
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
    
    // Sort nodes to find the best one to follow
    const sortedNodes = [...candidateNodes].sort((a, b) => {
        // For agents, prioritize by status
        if (a.type === 'agent' && b.type === 'agent') {
            const statusOrder = { 'active': 0, 'idle': 1, 'stale': 2 };
            const orderA = statusOrder[(a.status || 'unknown').toLowerCase()] ?? 3;
            const orderB = statusOrder[(b.status || 'unknown').toLowerCase()] ?? 3;
            
            if (orderA !== orderB) {
                return orderA - orderB;
            }
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
    
    // Get the top node to follow
    const targetNode = sortedNodes[0];
    
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
    
    // Check for stability to pause animation
    const hasAnimatedNodes = visibleNodes.some(node =>
        (node.status === 'in_progress' && node.type !== 'queue' && node.type !== 'agent' && node.type !== 'doc') ||
        (node.type === 'agent' && node.status === 'active') ||
        (node._departing && node.type === 'agent')
    );
    
    const hasAnimatedEdges = graphEdges.some(edge => {
        const edgeFilters = state.get('ui.edgeTypeFilters') || {};
        if (edgeFilters[edge.edge_type] === false) return false;
        if (!visibleNodeIds.has(edge.from) || !visibleNodeIds.has(edge.to)) return false;
        const style = getEdgeStyle(edge.edge_type);
        return style.animated;
    });
    
    if (!hasAnimatedNodes && !hasAnimatedEdges) {
        const isStable = visibleNodes.every(node => {
            const speed = Math.sqrt(node.vx * node.vx + node.vy * node.vy);
            return speed < STABLE_THRESHOLD;
        });
        
        if (isStable) {
            stableFrameCount++;
            if (stableFrameCount >= STABLE_FRAMES_REQUIRED) {
                isAnimating = false;
                return;
            }
        } else {
            stableFrameCount = 0;
        }
    }
    
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
    const isHovered = node === hoveredNode;
    const isDragging = node === draggedNode;
    const isSelected = selectedNodes.includes(node.id);
    const isMultiSelect = selectedNodes.length > 1;
    const zoom = getZoom();
    
    // Transform to screen coordinates
    const screenPos = worldToScreen(node.x, node.y, canvas);
    const radius = node.radius * zoom;
    
    // Get node color
    const color = getNodeColor(node);
    
    // Calculate opacity (for dimmed/fading nodes)
    let opacity = 1.0;
    if (node._departing && node.type === 'agent') {
        const departureTime = departingAgents.get(node.id);
        if (departureTime) {
            const elapsed = performance.now() - departureTime;
            const progress = Math.min(elapsed / AGENT_DEPARTURE_FADE_MS, 1.0);
            opacity = 1.0 - progress;
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
    
    // Draw animated rings for in_progress tasks/bugs/ideas
    if (node.status === 'in_progress' && node.type !== 'queue' && node.type !== 'agent' && node.type !== 'doc') {
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
    
    // Draw IDEA label inside idea cloud nodes
    drawIdeaLabel(node, screenPos, radius);
    
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
    
    // Calculate fade opacity for goodbye
    let fadeAlpha = 1.0;
    if (agent.goodbye_at) {
        const goodbyeTime = new Date(agent.goodbye_at).getTime();
        const elapsed = Date.now() - goodbyeTime;
        const fadeDuration = 5000;
        fadeAlpha = Math.max(0, 1 - (elapsed / fadeDuration));
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
 * Draw PRD label above doc nodes that are PRDs
 */
function drawPRDLabel(node, screenPos, radius) {
    if (node.type !== 'doc' || node.doc_type !== 'prd') return;
    
    const zoom = getZoom();
    const baseFontSize = 13 * Math.max(0.7, Math.min(1.3, zoom));
    const pillPadding = 6 * zoom;
    const pillHeight = baseFontSize + pillPadding * 2;
    const pillY = screenPos.y - radius - pillHeight - 8 * zoom;
    
    const displayText = 'PRD';
    
    ctx.font = `bold ${baseFontSize}px sans-serif`;
    const textWidth = ctx.measureText(displayText).width;
    const pillWidth = textWidth + pillPadding * 4;
    
    ctx.save();
    
    // Draw pill background
    ctx.fillStyle = 'rgba(147, 51, 234, 0.95)';  // Purple background
    ctx.beginPath();
    const pillX = screenPos.x - pillWidth / 2;
    const pillRadius = pillHeight / 2;
    ctx.roundRect(pillX, pillY, pillWidth, pillHeight, pillRadius);
    ctx.fill();
    
    // Draw text
    ctx.fillStyle = '#ffffff';
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.fillText(displayText, screenPos.x, pillY + pillHeight / 2);
    
    ctx.restore();
}

/**
 * Draw IDEA label inside idea cloud nodes
 */
function drawIdeaLabel(node, screenPos, _radius) {
    if (node.type !== 'idea') return;
    
    const zoom = getZoom();
    const baseFontSize = 11 * Math.max(0.7, Math.min(1.3, zoom));
    
    const displayText = 'IDEA';
    
    ctx.save();
    
    // Draw text centered in the cloud
    ctx.fillStyle = 'rgba(26, 35, 50, 0.5)';  // Semi-transparent dark text
    ctx.font = `bold ${baseFontSize}px sans-serif`;
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    
    // Position at top third of node to leave room for short_name below
    const labelY = screenPos.y - 10 * zoom;
    ctx.fillText(displayText, screenPos.x, labelY);
    
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
