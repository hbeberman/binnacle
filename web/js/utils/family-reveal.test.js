/**
 * Tests for family-reveal.js
 */

import { findFamilyRoot } from './family-reveal.js';

// Simple test helpers
function test(name, fn) {
    try {
        fn();
        console.log(`✓ ${name}`);
    } catch (error) {
        console.error(`✗ ${name}`);
        console.error(error);
    }
}

function assertEquals(actual, expected, message) {
    if (actual !== expected) {
        throw new Error(message || `Expected: ${expected}, Got: ${actual}`);
    }
}

function assertNull(actual, message) {
    if (actual !== null) {
        throw new Error(message || `Expected null, Got: ${actual}`);
    }
}

// Mock state module
let mockNodes = new Map();
let mockEdges = [];

// Override imports from state.js for testing
const originalImport = globalThis.__importMap;
globalThis.__getNode = (id) => mockNodes.get(id) || null;
globalThis.__getEdges = () => mockEdges;

function setupMocks() {
    mockNodes = new Map();
    mockEdges = [];
}

// Run tests
console.log('Running family-reveal tests...\n');

test('returns PRD when clicking task with PRD ancestor', () => {
    setupMocks();
        // Setup: task -> PRD
        mockNodes.set('bn-task', { id: 'bn-task', type: 'task' });
        mockNodes.set('bn-prd', { id: 'bn-prd', type: 'doc', doc_type: 'prd' });
        mockEdges = [
            { source: 'bn-task', target: 'bn-prd', edge_type: 'child_of' }
        ];
        
        const root = findFamilyRoot('bn-task');
        expect(root).toBe('bn-prd');
    });
    
    test('returns PRD when clicking PRD itself', () => {
        mockNodes.set('bn-prd', { id: 'bn-prd', type: 'doc', doc_type: 'prd' });
        
        const root = findFamilyRoot('bn-prd');
        expect(root).toBe('bn-prd');
    });
    
    test('returns milestone when no PRD exists', () => {
        // Setup: task -> milestone
        mockNodes.set('bn-task', { id: 'bn-task', type: 'task' });
        mockNodes.set('bn-milestone', { id: 'bn-milestone', type: 'milestone' });
        mockEdges = [
            { source: 'bn-task', target: 'bn-milestone', edge_type: 'child_of' }
        ];
        
        const root = findFamilyRoot('bn-task');
        expect(root).toBe('bn-milestone');
    });
    
    test('returns null for orphan node (no PRD or milestone)', () => {
        mockNodes.set('bn-orphan', { id: 'bn-orphan', type: 'task' });
        
        const root = findFamilyRoot('bn-orphan');
        expect(root).toBeNull();
    });
    
    test('prefers PRD over milestone when both exist', () => {
        // Setup: task -> milestone -> PRD
        mockNodes.set('bn-task', { id: 'bn-task', type: 'task' });
        mockNodes.set('bn-milestone', { id: 'bn-milestone', type: 'milestone' });
        mockNodes.set('bn-prd', { id: 'bn-prd', type: 'doc', doc_type: 'prd' });
        mockEdges = [
            { source: 'bn-task', target: 'bn-milestone', edge_type: 'child_of' },
            { source: 'bn-milestone', target: 'bn-prd', edge_type: 'child_of' }
        ];
        
        const root = findFamilyRoot('bn-task');
        expect(root).toBe('bn-prd');
    });
    
    test('handles deep hierarchy', () => {
        // Setup: task1 -> task2 -> task3 -> PRD
        mockNodes.set('bn-task1', { id: 'bn-task1', type: 'task' });
        mockNodes.set('bn-task2', { id: 'bn-task2', type: 'task' });
        mockNodes.set('bn-task3', { id: 'bn-task3', type: 'task' });
        mockNodes.set('bn-prd', { id: 'bn-prd', type: 'doc', doc_type: 'prd' });
        mockEdges = [
            { source: 'bn-task1', target: 'bn-task2', edge_type: 'child_of' },
            { source: 'bn-task2', target: 'bn-task3', edge_type: 'child_of' },
            { source: 'bn-task3', target: 'bn-prd', edge_type: 'child_of' }
        ];
        
        const root = findFamilyRoot('bn-task1');
        expect(root).toBe('bn-prd');
    });
    
    test('stops at PRD and does not continue upward', () => {
        // Setup: task -> PRD1 -> PRD2
        mockNodes.set('bn-task', { id: 'bn-task', type: 'task' });
        mockNodes.set('bn-prd1', { id: 'bn-prd1', type: 'doc', doc_type: 'prd' });
        mockNodes.set('bn-prd2', { id: 'bn-prd2', type: 'doc', doc_type: 'prd' });
        mockEdges = [
            { source: 'bn-task', target: 'bn-prd1', edge_type: 'child_of' },
            { source: 'bn-prd1', target: 'bn-prd2', edge_type: 'child_of' }
        ];
        
        const root = findFamilyRoot('bn-task');
        expect(root).toBe('bn-prd1');
    });
    
    test('prevents infinite loops with cycle detection', () => {
        // Setup: task1 -> task2 -> task1 (cycle)
        mockNodes.set('bn-task1', { id: 'bn-task1', type: 'task' });
        mockNodes.set('bn-task2', { id: 'bn-task2', type: 'task' });
        mockEdges = [
            { source: 'bn-task1', target: 'bn-task2', edge_type: 'child_of' },
            { source: 'bn-task2', target: 'bn-task1', edge_type: 'child_of' }
        ];
        
        const root = findFamilyRoot('bn-task1');
        expect(root).toBeNull();
    });
    
    test('ignores non-child_of edges', () => {
        // Setup: task -depends_on-> PRD (should not be followed)
        mockNodes.set('bn-task', { id: 'bn-task', type: 'task' });
        mockNodes.set('bn-prd', { id: 'bn-prd', type: 'doc', doc_type: 'prd' });
        mockEdges = [
            { source: 'bn-task', target: 'bn-prd', edge_type: 'depends_on' }
        ];
        
        const root = findFamilyRoot('bn-task');
        expect(root).toBeNull();
    });
    
    test('handles missing node gracefully', () => {
        // Edge points to non-existent node
        mockNodes.set('bn-task', { id: 'bn-task', type: 'task' });
        mockEdges = [
            { source: 'bn-task', target: 'bn-missing', edge_type: 'child_of' }
        ];
        
        const root = findFamilyRoot('bn-task');
        expect(root).toBeNull();
    });
    
    test('returns null for non-existent starting node', () => {
        const root = findFamilyRoot('bn-nonexistent');
        expect(root).toBeNull();
    });
    
    test('returns last milestone when multiple milestones but no PRD', () => {
        // Setup: task -> milestone1 -> milestone2
        mockNodes.set('bn-task', { id: 'bn-task', type: 'task' });
        mockNodes.set('bn-m1', { id: 'bn-m1', type: 'milestone' });
        mockNodes.set('bn-m2', { id: 'bn-m2', type: 'milestone' });
        mockEdges = [
            { source: 'bn-task', target: 'bn-m1', edge_type: 'child_of' },
            { source: 'bn-m1', target: 'bn-m2', edge_type: 'child_of' }
        ];
        
        const root = findFamilyRoot('bn-task');
        expect(root).toBe('bn-m2');
    });
});
