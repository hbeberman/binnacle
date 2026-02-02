/**
 * Doc Viewer Overlay Component
 * 
 * Full-screen overlay for viewing document content with:
 * - Header with document title and close button
 * - Scrollable content area for rendered markdown
 */

import { getNode, viewNodeOnGraph } from '../state.js';
import { renderMarkdown } from '../utils/markdown.js';
import { setupEntityLinkHover } from './tooltip.js';

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
                <div class="doc-viewer-header-left">
                    <h2 class="doc-viewer-title" id="doc-viewer-title">Document</h2>
                    <span class="doc-viewer-status" id="doc-viewer-status"></span>
                </div>
                <button class="doc-viewer-close" id="doc-viewer-close" title="Close">&times;</button>
            </div>
            <div class="doc-viewer-meta" id="doc-viewer-meta"></div>
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
export async function showDocViewer(docId) {
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
    
    // Update status badge for PRD docs
    const statusEl = document.getElementById('doc-viewer-status');
    if (node.doc_type === 'prd' && node.status) {
        const isDraft = node.status === 'draft';
        statusEl.textContent = isDraft ? 'DRAFT' : 'APPROVED';
        statusEl.className = `doc-viewer-status ${isDraft ? 'status-draft' : 'status-approved'}`;
        statusEl.style.display = 'inline-block';
    } else {
        statusEl.style.display = 'none';
    }
    
    // Clear meta section initially
    const metaEl = document.getElementById('doc-viewer-meta');
    metaEl.innerHTML = '';
    
    // Update content - render markdown
    const contentEl = document.getElementById('doc-viewer-content');
    
    // Show loading state
    contentEl.innerHTML = '<div class="doc-viewer-loading">Loading document...</div>';
    
    // Show overlay immediately with loading state
    overlay.classList.remove('hidden');
    
    // Fetch full document content
    try {
        const response = await fetch(`/api/docs/${docId}`);
        if (!response.ok) {
            throw new Error(`Failed to fetch document: ${response.status}`);
        }
        const data = await response.json();
        const fullDoc = data.doc;
        
        // Update approval metadata if available
        if (fullDoc.approval) {
            let metaHtml = '<div class="doc-viewer-approval">';
            if (fullDoc.approval.approved_by) {
                metaHtml += `<span class="approval-by">Approved by <strong>${escapeHtml(fullDoc.approval.approved_by)}</strong></span>`;
            }
            if (fullDoc.approval.approved_at) {
                const date = new Date(fullDoc.approval.approved_at);
                metaHtml += ` <span class="approval-date">on ${date.toLocaleDateString()}</span>`;
            }
            if (fullDoc.approval.reason) {
                metaHtml += ` <span class="approval-reason">— "${escapeHtml(fullDoc.approval.reason)}"</span>`;
            }
            metaHtml += '</div>';
            metaEl.innerHTML = metaHtml;
        }
        
        if (fullDoc.content) {
            // Render the markdown content
            renderMarkdown(contentEl, fullDoc.content);
        } else {
            // Show placeholder if no content
            contentEl.innerHTML = `
                <div class="doc-viewer-placeholder">
                    <p class="doc-viewer-note">This document has no content yet.</p>
                </div>
            `;
        }
    } catch (error) {
        console.error('Error loading document:', error);
        contentEl.innerHTML = `
            <div class="doc-viewer-placeholder">
                <p class="doc-viewer-note">Error loading document. Please try again.</p>
            </div>
        `;
    }
}

/**
 * Escape HTML characters to prevent XSS
 * @param {string} str - String to escape
 * @returns {string} Escaped string
 */
function escapeHtml(str) {
    if (!str) return '';
    return str
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        .replace(/"/g, '&quot;')
        .replace(/'/g, '&#039;');
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
        if (entityId) {
            viewNodeOnGraph(entityId);
        }
    });
    
    // Setup entity link tooltips
    setupEntityLinkHover(contentEl);
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
