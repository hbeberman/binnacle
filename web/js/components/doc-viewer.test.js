/**
 * Unit tests for Doc Viewer Overlay Component
 */

import { describe, it, expect, beforeEach, afterEach } from '../test-component.js';

describe('Doc Viewer Overlay', () => {
    let container;
    
    beforeEach(() => {
        // Create a fresh container for each test
        container = document.createElement('div');
        container.id = 'test-container';
        document.body.appendChild(container);
    });
    
    afterEach(() => {
        // Clean up
        if (container && container.parentNode) {
            container.parentNode.removeChild(container);
        }
    });
    
    it('should create overlay with correct structure', async () => {
        const { createDocViewer } = await import('./doc-viewer.js');
        const overlay = createDocViewer();
        
        expect(overlay).toBeDefined();
        expect(overlay.className).toContain('doc-viewer-overlay');
        expect(overlay.className).toContain('hidden');
        
        // Check for header elements
        const title = overlay.querySelector('.doc-viewer-title');
        const closeBtn = overlay.querySelector('.doc-viewer-close');
        expect(title).toBeDefined();
        expect(closeBtn).toBeDefined();
        
        // Check for content area
        const content = overlay.querySelector('.doc-viewer-content');
        expect(content).toBeDefined();
    });
    
    it('should mount to DOM correctly', async () => {
        const { mountDocViewer } = await import('./doc-viewer.js');
        
        mountDocViewer(container);
        
        const overlay = container.querySelector('#doc-viewer');
        expect(overlay).toBeDefined();
        expect(overlay.classList.contains('hidden')).toBe(true);
    });
    
    it('should show and hide overlay', async () => {
        const { mountDocViewer, showDocViewer, hideDocViewer } = await import('./doc-viewer.js');
        const { setEntities } = await import('../state.js');
        
        // Setup test doc
        setEntities('docs', [
            { id: 'bn-test', type: 'doc', title: 'Test Doc' }
        ]);
        
        mountDocViewer(container);
        const overlay = container.querySelector('#doc-viewer');
        
        // Initially hidden
        expect(overlay.classList.contains('hidden')).toBe(true);
        
        // Show
        showDocViewer('bn-test');
        expect(overlay.classList.contains('hidden')).toBe(false);
        
        // Hide
        hideDocViewer();
        expect(overlay.classList.contains('hidden')).toBe(true);
    });
    
    it('should display document title correctly', async () => {
        const { mountDocViewer, showDocViewer } = await import('./doc-viewer.js');
        const { setEntities } = await import('../state.js');
        
        setEntities('docs', [
            { id: 'bn-doc1', type: 'doc', title: 'My Test Document' }
        ]);
        
        mountDocViewer(container);
        showDocViewer('bn-doc1');
        
        const titleEl = container.querySelector('#doc-viewer-title');
        expect(titleEl.textContent).toBe('My Test Document');
    });
    
    it('should close on close button click', async () => {
        const { mountDocViewer, showDocViewer } = await import('./doc-viewer.js');
        const { setEntities } = await import('../state.js');
        
        setEntities('docs', [
            { id: 'bn-doc1', type: 'doc', title: 'Test' }
        ]);
        
        mountDocViewer(container);
        showDocViewer('bn-doc1');
        
        const overlay = container.querySelector('#doc-viewer');
        const closeBtn = container.querySelector('#doc-viewer-close');
        
        expect(overlay.classList.contains('hidden')).toBe(false);
        
        closeBtn.click();
        
        expect(overlay.classList.contains('hidden')).toBe(true);
    });
    
    it('should navigate graph when entity link clicked', async () => {
        const { mountDocViewer, showDocViewer } = await import('./doc-viewer.js');
        const { setEntities, getState } = await import('../state.js');
        
        // Setup test doc with content containing entity IDs
        setEntities('docs', [
            { 
                id: 'bn-doc1', 
                type: 'doc', 
                title: 'Test Doc',
                content: 'Related to task bn-1234 and bug bn-5678.'
            }
        ]);
        
        // Setup entities in graph
        setEntities('tasks', [
            { id: 'bn-1234', type: 'task', title: 'Test Task', x: 100, y: 200 }
        ]);
        
        mountDocViewer(container);
        showDocViewer('bn-doc1');
        
        // Find the entity link
        const entityLink = container.querySelector('.clickable-entity-id[data-entity-id="bn-1234"]');
        expect(entityLink).toBeDefined();
        
        // Click the entity link
        entityLink.click();
        
        // Check that the view switched to graph and node was selected
        const state = getState();
        expect(state.ui.currentView).toBe('graph');
        expect(state.ui.selectedNode).toBe('bn-1234');
    });
});

console.log('✓ Doc Viewer tests defined');
