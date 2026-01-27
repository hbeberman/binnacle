/**
 * Zoom Controls Component
 * 
 * Provides zoom in/out buttons and zoom level display for the graph view
 */

import * as graph from '../graph/index.js';

/**
 * Create zoom controls overlay
 * @returns {HTMLElement} Zoom controls container
 */
export function createZoomControls() {
    const controls = document.createElement('div');
    controls.className = 'zoom-controls-container';
    
    controls.innerHTML = `
        <div class="zoom-controls">
            <button class="graph-control-btn zoom-in-btn" title="Zoom In">+</button>
            <div class="zoom-level" id="zoom-level">100%</div>
            <button class="graph-control-btn zoom-out-btn" title="Zoom Out">−</button>
        </div>
    `;
    
    return controls;
}

/**
 * Initialize zoom controls with event handlers
 * @param {HTMLElement} controls - The zoom controls element
 */
export function initializeZoomControls(controls) {
    const zoomInBtn = controls.querySelector('.zoom-in-btn');
    const zoomOutBtn = controls.querySelector('.zoom-out-btn');
    const zoomLevel = controls.querySelector('#zoom-level');
    
    // Zoom in handler
    if (zoomInBtn) {
        zoomInBtn.addEventListener('click', () => {
            graph.zoomIn();
            updateZoomLevel(zoomLevel);
        });
    }
    
    // Zoom out handler
    if (zoomOutBtn) {
        zoomOutBtn.addEventListener('click', () => {
            graph.zoomOut();
            updateZoomLevel(zoomLevel);
        });
    }
    
    // Update zoom level display initially and on viewport changes
    updateZoomLevel(zoomLevel);
    
    // Listen for viewport changes to update zoom level
    // (Could use state subscription if available)
}

/**
 * Update zoom level display
 * @param {HTMLElement} zoomLevel - The zoom level display element
 */
function updateZoomLevel(zoomLevel) {
    if (!zoomLevel) return;
    
    const zoom = graph.getZoom ? graph.getZoom() : 1.0;
    const percentage = Math.round(zoom * 100);
    zoomLevel.textContent = `${percentage}%`;
}

/**
 * Mount zoom controls to graph view
 * @param {HTMLElement} graphView - The graph view container
 * @returns {HTMLElement} The created zoom controls element
 */
export function mountZoomControls(graphView) {
    const controls = createZoomControls();
    graphView.appendChild(controls);
    initializeZoomControls(controls);
    return controls;
}
