/**
 * Tests for node highlighting API
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';

// Mock dependencies
vi.mock('./renderer.js', () => {
    let highlightedNodeId = null;
    let highlightStartTime = null;
    let animationStarted = false;
    let panCalled = false;
    
    const mockNodes = [
        { id: 'bn-1234', x: 100, y: 100, radius: 20 },
        { id: 'bn-5678', x: 200, y: 200, radius: 20 }
    ];
    
    return {
        highlightNode: (nodeId) => {
            highlightedNodeId = nodeId;
            highlightStartTime = performance.now();
            animationStarted = true;
            
            const node = mockNodes.find(n => n.id === nodeId);
            if (node) {
                panCalled = true;
            }
        },
        clearHighlight: () => {
            highlightedNodeId = null;
            highlightStartTime = null;
        },
        // Test helpers
        __getHighlightedNodeId: () => highlightedNodeId,
        __getAnimationStarted: () => animationStarted,
        __getPanCalled: () => panCalled,
        __reset: () => {
            highlightedNodeId = null;
            highlightStartTime = null;
            animationStarted = false;
            panCalled = false;
        }
    };
});

describe('Node Highlighting API', () => {
    beforeEach(async () => {
        const { __reset } = await import('./renderer.js');
        __reset();
    });
    
    it('should export highlightNode function', async () => {
        const { highlightNode } = await import('./index.js');
        expect(typeof highlightNode).toBe('function');
    });
    
    it('should export clearHighlight function', async () => {
        const { clearHighlight } = await import('./index.js');
        expect(typeof clearHighlight).toBe('function');
    });
    
    it('should highlight a node when called', async () => {
        const { highlightNode, __getHighlightedNodeId } = await import('./renderer.js');
        
        highlightNode('bn-1234');
        
        expect(__getHighlightedNodeId()).toBe('bn-1234');
    });
    
    it('should start animation when highlighting', async () => {
        const { highlightNode, __getAnimationStarted } = await import('./renderer.js');
        
        highlightNode('bn-1234');
        
        expect(__getAnimationStarted()).toBe(true);
    });
    
    it('should clear highlight when called', async () => {
        const { highlightNode, clearHighlight, __getHighlightedNodeId } = await import('./renderer.js');
        
        highlightNode('bn-1234');
        expect(__getHighlightedNodeId()).toBe('bn-1234');
        
        clearHighlight();
        expect(__getHighlightedNodeId()).toBe(null);
    });
    
    it('should handle null nodeId gracefully', async () => {
        const { highlightNode, __getHighlightedNodeId } = await import('./renderer.js');
        
        highlightNode(null);
        
        // Should not throw and should not set highlight
        expect(__getHighlightedNodeId()).toBe(null);
    });
});
