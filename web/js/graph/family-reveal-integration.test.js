import { describe, it, expect, beforeEach } from '@jest/globals';
import * as state from '../state.js';

// Mock the family-reveal functions for testing
const mockFindFamilyRoot = (nodeId) => {
    // Simulate finding a PRD root
    if (nodeId === 'bn-task1' || nodeId === 'bn-task2') {
        return 'bn-prd';
    }
    if (nodeId === 'bn-orphan') {
        return null;
    }
    return 'bn-prd';
};

const mockCollectDescendants = (rootId) => {
    // Simulate collecting descendants
    if (rootId === 'bn-prd') {
        return new Set(['bn-prd', 'bn-task1', 'bn-task2', 'bn-task3']);
    }
    return new Set([rootId]);
};

/**
 * Simulate the onNodeClick handler logic
 */
function simulateNodeClick(node) {
    const rootId = mockFindFamilyRoot(node.id);
    if (rootId) {
        const descendants = mockCollectDescendants(rootId);
        state.set('ui.familyReveal', {
            active: true,
            rootId: rootId,
            revealedNodeIds: descendants
        });
    } else {
        state.set('ui.familyReveal', {
            active: false,
            rootId: null,
            revealedNodeIds: new Set()
        });
    }
}

describe('Family Reveal Integration', () => {
    beforeEach(() => {
        // Reset state before each test
        state.set('ui.familyReveal', {
            active: false,
            rootId: null,
            revealedNodeIds: new Set()
        });
    });

    it('should activate family reveal when clicking a node with a root', () => {
        simulateNodeClick({ id: 'bn-task1' });
        
        const familyReveal = state.get('ui.familyReveal');
        expect(familyReveal.active).toBe(true);
        expect(familyReveal.rootId).toBe('bn-prd');
        expect(familyReveal.revealedNodeIds.size).toBe(4);
        expect(familyReveal.revealedNodeIds.has('bn-prd')).toBe(true);
        expect(familyReveal.revealedNodeIds.has('bn-task1')).toBe(true);
    });

    it('should clear family reveal when clicking an orphan node', () => {
        // First set up some state
        state.set('ui.familyReveal', {
            active: true,
            rootId: 'bn-prd',
            revealedNodeIds: new Set(['bn-prd', 'bn-task1'])
        });
        
        // Click orphan node
        simulateNodeClick({ id: 'bn-orphan' });
        
        const familyReveal = state.get('ui.familyReveal');
        expect(familyReveal.active).toBe(false);
        expect(familyReveal.rootId).toBe(null);
        expect(familyReveal.revealedNodeIds.size).toBe(0);
    });

    it('should update family reveal when switching between different families', () => {
        // Click first task
        simulateNodeClick({ id: 'bn-task1' });
        
        let familyReveal = state.get('ui.familyReveal');
        expect(familyReveal.active).toBe(true);
        expect(familyReveal.rootId).toBe('bn-prd');
        
        // Click second task in same family
        simulateNodeClick({ id: 'bn-task2' });
        
        familyReveal = state.get('ui.familyReveal');
        expect(familyReveal.active).toBe(true);
        expect(familyReveal.rootId).toBe('bn-prd');
        expect(familyReveal.revealedNodeIds.size).toBe(4);
    });

    it('should track revealed nodes in state', () => {
        simulateNodeClick({ id: 'bn-task1' });
        
        const familyReveal = state.get('ui.familyReveal');
        const revealedIds = familyReveal.revealedNodeIds;
        
        expect(revealedIds instanceof Set).toBe(true);
        expect(revealedIds.has('bn-task1')).toBe(true);
        expect(revealedIds.has('bn-task2')).toBe(true);
        expect(revealedIds.has('bn-task3')).toBe(true);
    });
});
