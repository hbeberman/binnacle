/**
 * Doc Viewer Overlay Component
 * 
 * Full-screen overlay for viewing document content with:
 * - Header with document title and close button
 * - Scrollable content area for rendered markdown
 */

import { getNode, setSelectedNode, setCurrentView } from '../state.js';
import { renderMarkdown } from '../utils/markdown.js';
import { panToNode } from '../graph/index.js';

/**
 * Create the doc viewer overlay HTML
 * @returns {HTMLElement} The doc viewer overlay element
 */
export function createDocViewer() {
    const overlay = document.createElement('div');
    overlay.className = 'doc-viewer-overlay hidden';
    overlay.id = 'doc-viewer';
    
    overlay.innerHTML = `
        <div class="doc-viewer">
            <div class="doc-viewer-header">
                <h2 class="doc-viewer-title" id="doc-viewer-title">Document</h2>
                <button class="doc-viewer-close" id="doc-viewer-close" title="Close">&times;</button>
            </div>
            <div class="doc-viewer-content" id="doc-viewer-content">
                <div class="doc-viewer-loading">Loading document...</div>
            </div>
        </div>
    `;
    
    return overlay;
}

/**
 * Show the doc viewer overlay with document content
 * @param {string} docId - The document node ID
 */
export function showDocViewer(docId) {
    const overlay = document.getElementById('doc-viewer');
    if (!overlay) {
        console.error('Doc viewer overlay not found in DOM');
        return;
    }
    
    const node = getNode(docId);
    if (!node) {
        console.error(`Document node ${docId} not found`);
        return;
    }
    
    // Update title
    const titleEl = document.getElementById('doc-viewer-title');
    titleEl.textContent = node.title || docId;
    
    // Update content - render markdown
    const contentEl = document.getElementById('doc-viewer-content');
    
    if (node.content) {
        // Render the markdown content
        renderMarkdown(contentEl, node.content);
    } else {
        // Show placeholder if no content
        contentEl.innerHTML = `
            <div class="doc-viewer-placeholder">
                <p class="doc-viewer-note">This document has no content yet.</p>
            </div>
        `;
    }
    
    // Show overlay
    overlay.classList.remove('hidden');
}

/**
 * Hide the doc viewer overlay
 */
export function hideDocViewer() {
    const overlay = document.getElementById('doc-viewer');
    if (overlay) {
        overlay.classList.add('hidden');
    }
}

/**
 * Initialize the doc viewer with event handlers
 */
export function initDocViewer() {
    const overlay = document.getElementById('doc-viewer');
    if (!overlay) {
        console.error('Doc viewer overlay not found in DOM');
        return;
    }
    
    // Close button
    const closeBtn = document.getElementById('doc-viewer-close');
    closeBtn.addEventListener('click', hideDocViewer);
    
    // Close on overlay click (but not on content click)
    overlay.addEventListener('click', (e) => {
        if (e.target === overlay) {
            hideDocViewer();
        }
    });
    
    // Close on Escape key
    document.addEventListener('keydown', (e) => {
        if (e.key === 'Escape' && !overlay.classList.contains('hidden')) {
            hideDocViewer();
        }
    });
    
    // Entity ID click handler - navigate graph to clicked entity
    const contentEl = document.getElementById('doc-viewer-content');
    contentEl.addEventListener('click', (e) => {
        const clickableId = e.target.closest('.clickable-entity-id');
        if (!clickableId) return;
        
        e.preventDefault();
        e.stopPropagation();
        
        const entityId = clickableId.dataset.entityId;
        if (!entityId) return;
        
        // Get the node to find its position
        const node = getNode(entityId);
        if (!node) {
            console.warn(`Entity ${entityId} not found`);
            return;
        }
        
        // Switch to graph view
        setCurrentView('graph');
        
        // Pan to the node's position if coordinates exist
        if (typeof node.x === 'number' && typeof node.y === 'number') {
            panToNode(node.x, node.y, {
                duration: 500,
                zoom: 1.5
            });
        }
        
        // Select the node
        setSelectedNode(entityId);
    });
}

/**
 * Mount the doc viewer to the DOM
 * @param {HTMLElement|string} target - Target element or selector
 */
export function mountDocViewer(target) {
    const container = typeof target === 'string' 
        ? document.querySelector(target) 
        : target;
    
    if (!container) {
        console.error('Doc viewer target not found');
        return;
    }
    
    const viewer = createDocViewer();
    container.appendChild(viewer);
    initDocViewer();
}
