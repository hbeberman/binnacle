/**
 * Markdown Pane Component
 * 
 * A fixed-width side panel for displaying markdown content with:
 * - Header with document title and close button
 * - Scrollable content area for rendered markdown
 * - Fixed 450px width
 * - Entity ID link integration for graph navigation
 */

import { getNode, setSelectedNode, setCurrentView } from '../state.js';
import { renderMarkdown } from '../utils/markdown.js';
import { panToNode } from '../graph/index.js';

/**
 * Create the markdown pane HTML
 * @returns {HTMLElement} The markdown pane element
 */
export function createMarkdownPane() {
    const pane = document.createElement('div');
    pane.className = 'markdown-pane hidden';
    pane.id = 'markdown-pane';
    
    pane.innerHTML = `
        <div class="markdown-pane-header">
            <h2 class="markdown-pane-title" id="markdown-pane-title">Document</h2>
            <button class="markdown-pane-close" id="markdown-pane-close" title="Close">&times;</button>
        </div>
        <div class="markdown-pane-content" id="markdown-pane-content">
            <div class="markdown-pane-loading">Loading document...</div>
        </div>
    `;
    
    return pane;
}

/**
 * Show the markdown pane with document content
 * @param {string} docId - The document node ID
 */
export function showMarkdownPane(docId) {
    const pane = document.getElementById('markdown-pane');
    if (!pane) {
        console.error('Markdown pane not found in DOM');
        return;
    }
    
    const node = getNode(docId);
    if (!node) {
        console.error(`Document node ${docId} not found`);
        return;
    }
    
    // Update title
    const titleEl = document.getElementById('markdown-pane-title');
    titleEl.textContent = node.title || docId;
    
    // Update content - render markdown
    const contentEl = document.getElementById('markdown-pane-content');
    
    if (node.content) {
        // Render the markdown content
        renderMarkdown(contentEl, node.content);
    } else {
        // Show placeholder if no content
        contentEl.innerHTML = `
            <div class="markdown-pane-placeholder">
                <p class="markdown-pane-note">This document has no content yet.</p>
            </div>
        `;
    }
    
    // Show pane
    pane.classList.remove('hidden');
}

/**
 * Hide the markdown pane
 */
export function hideMarkdownPane() {
    const pane = document.getElementById('markdown-pane');
    if (pane) {
        pane.classList.add('hidden');
    }
}

/**
 * Initialize the markdown pane with event handlers
 */
function initMarkdownPane() {
    const pane = document.getElementById('markdown-pane');
    if (!pane) {
        console.error('Markdown pane not found in DOM');
        return;
    }
    
    // Close button
    const closeBtn = document.getElementById('markdown-pane-close');
    if (closeBtn) {
        closeBtn.addEventListener('click', hideMarkdownPane);
    }
    
    // Close on Escape key
    document.addEventListener('keydown', (e) => {
        if (e.key === 'Escape' && !pane.classList.contains('hidden')) {
            hideMarkdownPane();
        }
    });
    
    // Entity ID click handler - navigate graph to clicked entity
    const contentEl = document.getElementById('markdown-pane-content');
    if (contentEl) {
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
                    targetZoom: 1.5
                });
            }
            
            // Select the node
            setSelectedNode(entityId);
        });
    }
}

/**
 * Mount the markdown pane to the DOM
 * @param {HTMLElement|string} target - Target element or selector
 */
export function mountMarkdownPane(target) {
    const container = typeof target === 'string' 
        ? document.querySelector(target) 
        : target;
    
    if (!container) {
        console.error('Markdown pane target not found');
        return;
    }
    
    const pane = createMarkdownPane();
    container.appendChild(pane);
    initMarkdownPane();
}
