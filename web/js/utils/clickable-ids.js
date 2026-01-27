/**
 * Clickable IDs Utility
 * 
 * Makes binnacle IDs (bn-xxxx, bnt-xxxx, etc.) clickable and navigates
 * to their location in the graph view.
 */

import * as state from '../state.js';
import { panToNode } from '../graph/index.js';
import { getNodes } from '../graph/index.js';

/**
 * Regex pattern to match binnacle IDs
 * Matches: bn-xxxx, bnt-xxxx, bnq-xxxx, etc.
 */
const BINNACLE_ID_PATTERN = /\b(bn[a-z]?-[a-f0-9]{4})\b/gi;

/**
 * Navigate to a node in the graph view
 * @param {string} nodeId - The binnacle ID to navigate to
 */
export function navigateToNode(nodeId) {
    // Find the node in graph nodes (which have x, y coordinates)
    const graphNodes = getNodes();
    const node = graphNodes.find(n => n.id === nodeId);
    
    if (!node) {
        console.warn(`Node ${nodeId} not found in graph`);
        return;
    }
    
    // Switch to graph view if not already there
    const currentView = state.get('ui.currentView');
    if (currentView !== 'graph') {
        state.set('ui.currentView', 'graph');
    }
    
    // Pan to the node's position
    const canvas = document.querySelector('#graph-canvas');
    panToNode(node.x, node.y, { 
        canvas,
        duration: 500
    });
    
    // Optionally, select the node to show its details
    state.set('ui.selectedNode', nodeId);
}

/**
 * Make a text element's binnacle IDs clickable
 * Replaces plain text IDs with clickable links
 * @param {HTMLElement} element - The element containing text with IDs
 */
export function makeIdsClickable(element) {
    if (!element || !element.textContent) return;
    
    const text = element.textContent;
    
    // Check if text contains any binnacle IDs
    if (!BINNACLE_ID_PATTERN.test(text)) {
        return;
    }
    
    // Reset regex lastIndex
    BINNACLE_ID_PATTERN.lastIndex = 0;
    
    // Split text by IDs and rebuild with clickable spans
    const parts = [];
    let lastIndex = 0;
    let match;
    
    while ((match = BINNACLE_ID_PATTERN.exec(text)) !== null) {
        // Add text before the match
        if (match.index > lastIndex) {
            parts.push(document.createTextNode(text.slice(lastIndex, match.index)));
        }
        
        // Create clickable span for the ID
        const idSpan = document.createElement('span');
        idSpan.className = 'clickable-id';
        idSpan.textContent = match[0];
        idSpan.dataset.nodeId = match[0];
        idSpan.title = `Click to navigate to ${match[0]}`;
        idSpan.style.cursor = 'pointer';
        idSpan.style.textDecoration = 'underline';
        idSpan.style.color = 'var(--accent-blue, #4a9eff)';
        
        idSpan.addEventListener('click', (e) => {
            e.preventDefault();
            e.stopPropagation();
            navigateToNode(match[0]);
        });
        
        parts.push(idSpan);
        lastIndex = BINNACLE_ID_PATTERN.lastIndex;
    }
    
    // Add remaining text
    if (lastIndex < text.length) {
        parts.push(document.createTextNode(text.slice(lastIndex)));
    }
    
    // Clear element and add new content
    element.textContent = '';
    parts.forEach(part => element.appendChild(part));
}

/**
 * Create a clickable ID element
 * @param {string} nodeId - The binnacle ID
 * @returns {HTMLElement} A clickable span element
 */
export function createClickableId(nodeId) {
    const span = document.createElement('span');
    span.className = 'clickable-id';
    span.textContent = nodeId;
    span.dataset.nodeId = nodeId;
    span.title = `Click to navigate to ${nodeId}`;
    span.style.cursor = 'pointer';
    span.style.textDecoration = 'underline';
    span.style.color = 'var(--accent-blue, #4a9eff)';
    
    span.addEventListener('click', (e) => {
        e.preventDefault();
        e.stopPropagation();
        navigateToNode(nodeId);
    });
    
    return span;
}
