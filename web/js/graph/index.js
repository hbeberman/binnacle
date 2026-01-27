/**
 * Binnacle Graph Module
 * 
 * Main entry point for the graph canvas rendering system.
 * Re-exports all public APIs from submodules.
 */

// Re-export from renderer (main API)
export {
    init,
    resizeCanvas,
    startAnimation,
    stopAnimation,
    setHoveredNode,
    setDraggedNode,
    findNodeAtPosition,
    findEdgeAtPosition,
    moveNode,
    getNodes,
    getVisibleNodes,
    getCanvas,
    markAgentDeparting,
    highlightNode,
    clearHighlight,
    animationTime,
    rebuildGraph
} from './renderer.js';

// Re-export coordinate transformations
export {
    screenToWorld,
    worldToScreen,
    getZoom,
    getPan,
    applyZoom,
    applyPan,
    centerOn,
    resetViewport,
    getVisibleBounds,
    isInViewport,
    panToNode,
    cancelPanAnimation
} from './transform.js';

// Re-export shape drawing functions
export {
    drawHexagonPath,
    drawDiamondPath,
    drawRobotPath,
    drawSquarePath,
    drawCloudPath,
    drawDocPath,
    drawNodeShapePath
} from './shapes.js';

// Re-export color utilities
export {
    getNodeColor,
    getEdgeStyle,
    getCSSColors,
    TASK_STATUS_COLORS,
    BUG_STATUS_COLORS,
    IDEA_STATUS_COLORS,
    MILESTONE_STATUS_COLORS,
    AGENT_STATUS_COLORS,
    DOC_TYPE_COLORS,
    QUEUE_COLOR,
    TEST_STATUS_COLORS
} from './colors.js';

// Re-export camera controls
export {
    init as initCamera,
    zoomIn,
    zoomOut,
    resetCamera,
    resumeAutoFollow
} from './camera.js';
