/**
 * Tests for family-reveal.js
 */

import { findFamilyRoot, collectDescendants } from './family-reveal.js';

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

function assertSetEquals(actual, expected, message) {
    const actualArray = Array.from(actual).sort();
    const expectedArray = Array.from(expected).sort();
    if (JSON.stringify(actualArray) !== JSON.stringify(expectedArray)) {
        throw new Error(message || `Expected Set: ${JSON.stringify(expectedArray)}, Got: ${JSON.stringify(actualArray)}`);
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

// findFamilyRoot tests
test('findFamilyRoot: returns PRD when clicking task with PRD ancestor', () => {
    setupMocks();
    mockNodes.set('bn-task', { id: 'bn-task', type: 'task' });
    mockNodes.set('bn-prd', { id: 'bn-prd', type: 'doc', doc_type: 'prd' });
    mockEdges = [
        { source: 'bn-task', target: 'bn-prd', edge_type: 'child_of' }
    ];
    
    const root = findFamilyRoot('bn-task');
    assertEquals(root, 'bn-prd');
});

test('findFamilyRoot: returns PRD when clicking PRD itself', () => {
    setupMocks();
    mockNodes.set('bn-prd', { id: 'bn-prd', type: 'doc', doc_type: 'prd' });
    
    const root = findFamilyRoot('bn-prd');
    assertEquals(root, 'bn-prd');
});

test('findFamilyRoot: returns milestone when no PRD exists', () => {
    setupMocks();
    mockNodes.set('bn-task', { id: 'bn-task', type: 'task' });
    mockNodes.set('bn-milestone', { id: 'bn-milestone', type: 'milestone' });
    mockEdges = [
        { source: 'bn-task', target: 'bn-milestone', edge_type: 'child_of' }
    ];
    
    const root = findFamilyRoot('bn-task');
    assertEquals(root, 'bn-milestone');
});

test('findFamilyRoot: returns null for orphan node', () => {
    setupMocks();
    mockNodes.set('bn-orphan', { id: 'bn-orphan', type: 'task' });
    
    const root = findFamilyRoot('bn-orphan');
    assertNull(root);
});

test('findFamilyRoot: prefers PRD over milestone when both exist', () => {
    setupMocks();
    mockNodes.set('bn-task', { id: 'bn-task', type: 'task' });
    mockNodes.set('bn-milestone', { id: 'bn-milestone', type: 'milestone' });
    mockNodes.set('bn-prd', { id: 'bn-prd', type: 'doc', doc_type: 'prd' });
    mockEdges = [
        { source: 'bn-task', target: 'bn-milestone', edge_type: 'child_of' },
        { source: 'bn-milestone', target: 'bn-prd', edge_type: 'child_of' }
    ];
    
    const root = findFamilyRoot('bn-task');
    assertEquals(root, 'bn-prd');
});

test('findFamilyRoot: handles deep hierarchy', () => {
    setupMocks();
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
    assertEquals(root, 'bn-prd');
});

test('findFamilyRoot: stops at PRD and does not continue upward', () => {
    setupMocks();
    mockNodes.set('bn-task', { id: 'bn-task', type: 'task' });
    mockNodes.set('bn-prd1', { id: 'bn-prd1', type: 'doc', doc_type: 'prd' });
    mockNodes.set('bn-prd2', { id: 'bn-prd2', type: 'doc', doc_type: 'prd' });
    mockEdges = [
        { source: 'bn-task', target: 'bn-prd1', edge_type: 'child_of' },
        { source: 'bn-prd1', target: 'bn-prd2', edge_type: 'child_of' }
    ];
    
    const root = findFamilyRoot('bn-task');
    assertEquals(root, 'bn-prd1');
});

test('findFamilyRoot: prevents infinite loops with cycle detection', () => {
    setupMocks();
    mockNodes.set('bn-task1', { id: 'bn-task1', type: 'task' });
    mockNodes.set('bn-task2', { id: 'bn-task2', type: 'task' });
    mockEdges = [
        { source: 'bn-task1', target: 'bn-task2', edge_type: 'child_of' },
        { source: 'bn-task2', target: 'bn-task1', edge_type: 'child_of' }
    ];
    
    const root = findFamilyRoot('bn-task1');
    assertNull(root);
});

test('findFamilyRoot: ignores non-child_of edges', () => {
    setupMocks();
    mockNodes.set('bn-task', { id: 'bn-task', type: 'task' });
    mockNodes.set('bn-prd', { id: 'bn-prd', type: 'doc', doc_type: 'prd' });
    mockEdges = [
        { source: 'bn-task', target: 'bn-prd', edge_type: 'depends_on' }
    ];
    
    const root = findFamilyRoot('bn-task');
    assertNull(root);
});

test('findFamilyRoot: handles missing node gracefully', () => {
    setupMocks();
    mockNodes.set('bn-task', { id: 'bn-task', type: 'task' });
    mockEdges = [
        { source: 'bn-task', target: 'bn-missing', edge_type: 'child_of' }
    ];
    
    const root = findFamilyRoot('bn-task');
    assertNull(root);
});

test('findFamilyRoot: returns null for non-existent starting node', () => {
    setupMocks();
    const root = findFamilyRoot('bn-nonexistent');
    assertNull(root);
});

test('findFamilyRoot: returns last milestone when multiple milestones but no PRD', () => {
    setupMocks();
    mockNodes.set('bn-task', { id: 'bn-task', type: 'task' });
    mockNodes.set('bn-m1', { id: 'bn-m1', type: 'milestone' });
    mockNodes.set('bn-m2', { id: 'bn-m2', type: 'milestone' });
    mockEdges = [
        { source: 'bn-task', target: 'bn-m1', edge_type: 'child_of' },
        { source: 'bn-m1', target: 'bn-m2', edge_type: 'child_of' }
    ];
    
    const root = findFamilyRoot('bn-task');
    assertEquals(root, 'bn-m2');
});

// collectDescendants tests
test('collectDescendants: returns only root for node with no children', () => {
    setupMocks();
    mockNodes.set('bn-root', { id: 'bn-root', type: 'task' });
    
    const descendants = collectDescendants('bn-root');
    assertSetEquals(descendants, new Set(['bn-root']));
});

test('collectDescendants: collects immediate children', () => {
    setupMocks();
    mockNodes.set('bn-root', { id: 'bn-root', type: 'doc', doc_type: 'prd' });
    mockNodes.set('bn-child1', { id: 'bn-child1', type: 'task' });
    mockNodes.set('bn-child2', { id: 'bn-child2', type: 'task' });
    mockEdges = [
        { source: 'bn-child1', target: 'bn-root', edge_type: 'child_of' },
        { source: 'bn-child2', target: 'bn-root', edge_type: 'child_of' }
    ];
    
    const descendants = collectDescendants('bn-root');
    assertSetEquals(descendants, new Set(['bn-root', 'bn-child1', 'bn-child2']));
});

test('collectDescendants: collects multi-level hierarchy', () => {
    setupMocks();
    // Tree: root -> child1 -> grandchild1
    //            -> child2
    mockNodes.set('bn-root', { id: 'bn-root', type: 'doc', doc_type: 'prd' });
    mockNodes.set('bn-child1', { id: 'bn-child1', type: 'task' });
    mockNodes.set('bn-child2', { id: 'bn-child2', type: 'task' });
    mockNodes.set('bn-grandchild1', { id: 'bn-grandchild1', type: 'task' });
    mockEdges = [
        { source: 'bn-child1', target: 'bn-root', edge_type: 'child_of' },
        { source: 'bn-child2', target: 'bn-root', edge_type: 'child_of' },
        { source: 'bn-grandchild1', target: 'bn-child1', edge_type: 'child_of' }
    ];
    
    const descendants = collectDescendants('bn-root');
    assertSetEquals(descendants, new Set(['bn-root', 'bn-child1', 'bn-child2', 'bn-grandchild1']));
});

test('collectDescendants: handles deep tree', () => {
    setupMocks();
    // Deep tree: root -> n1 -> n2 -> n3 -> n4
    mockNodes.set('bn-root', { id: 'bn-root', type: 'milestone' });
    mockNodes.set('bn-n1', { id: 'bn-n1', type: 'task' });
    mockNodes.set('bn-n2', { id: 'bn-n2', type: 'task' });
    mockNodes.set('bn-n3', { id: 'bn-n3', type: 'task' });
    mockNodes.set('bn-n4', { id: 'bn-n4', type: 'task' });
    mockEdges = [
        { source: 'bn-n1', target: 'bn-root', edge_type: 'child_of' },
        { source: 'bn-n2', target: 'bn-n1', edge_type: 'child_of' },
        { source: 'bn-n3', target: 'bn-n2', edge_type: 'child_of' },
        { source: 'bn-n4', target: 'bn-n3', edge_type: 'child_of' }
    ];
    
    const descendants = collectDescendants('bn-root');
    assertSetEquals(descendants, new Set(['bn-root', 'bn-n1', 'bn-n2', 'bn-n3', 'bn-n4']));
});

test('collectDescendants: handles wide tree', () => {
    setupMocks();
    // Wide tree: root has 5 children
    mockNodes.set('bn-root', { id: 'bn-root', type: 'doc', doc_type: 'prd' });
    for (let i = 1; i <= 5; i++) {
        mockNodes.set(`bn-child${i}`, { id: `bn-child${i}`, type: 'task' });
        mockEdges.push({ source: `bn-child${i}`, target: 'bn-root', edge_type: 'child_of' });
    }
    
    const descendants = collectDescendants('bn-root');
    assertSetEquals(descendants, new Set(['bn-root', 'bn-child1', 'bn-child2', 'bn-child3', 'bn-child4', 'bn-child5']));
});

test('collectDescendants: prevents infinite loops with cycle detection', () => {
    setupMocks();
    // Cycle: root -> child1 -> child2 -> child1 (cycle back)
    mockNodes.set('bn-root', { id: 'bn-root', type: 'doc', doc_type: 'prd' });
    mockNodes.set('bn-child1', { id: 'bn-child1', type: 'task' });
    mockNodes.set('bn-child2', { id: 'bn-child2', type: 'task' });
    mockEdges = [
        { source: 'bn-child1', target: 'bn-root', edge_type: 'child_of' },
        { source: 'bn-child2', target: 'bn-child1', edge_type: 'child_of' },
        { source: 'bn-child1', target: 'bn-child2', edge_type: 'child_of' }  // Cycle
    ];
    
    const descendants = collectDescendants('bn-root');
    // Should collect all nodes exactly once despite cycle
    assertSetEquals(descendants, new Set(['bn-root', 'bn-child1', 'bn-child2']));
});

test('collectDescendants: ignores non-child_of edges', () => {
    setupMocks();
    mockNodes.set('bn-root', { id: 'bn-root', type: 'doc', doc_type: 'prd' });
    mockNodes.set('bn-child', { id: 'bn-child', type: 'task' });
    mockNodes.set('bn-dep', { id: 'bn-dep', type: 'task' });
    mockEdges = [
        { source: 'bn-child', target: 'bn-root', edge_type: 'child_of' },
        { source: 'bn-dep', target: 'bn-root', edge_type: 'depends_on' }  // Should be ignored
    ];
    
    const descendants = collectDescendants('bn-root');
    assertSetEquals(descendants, new Set(['bn-root', 'bn-child']));
});

test('collectDescendants: handles complex tree structure', () => {
    setupMocks();
    // Complex tree:
    //       root
    //      /    \
    //    c1      c2
    //   / \       \
    //  gc1 gc2    gc3
    mockNodes.set('bn-root', { id: 'bn-root', type: 'doc', doc_type: 'prd' });
    mockNodes.set('bn-c1', { id: 'bn-c1', type: 'task' });
    mockNodes.set('bn-c2', { id: 'bn-c2', type: 'task' });
    mockNodes.set('bn-gc1', { id: 'bn-gc1', type: 'task' });
    mockNodes.set('bn-gc2', { id: 'bn-gc2', type: 'task' });
    mockNodes.set('bn-gc3', { id: 'bn-gc3', type: 'task' });
    mockEdges = [
        { source: 'bn-c1', target: 'bn-root', edge_type: 'child_of' },
        { source: 'bn-c2', target: 'bn-root', edge_type: 'child_of' },
        { source: 'bn-gc1', target: 'bn-c1', edge_type: 'child_of' },
        { source: 'bn-gc2', target: 'bn-c1', edge_type: 'child_of' },
        { source: 'bn-gc3', target: 'bn-c2', edge_type: 'child_of' }
    ];
    
    const descendants = collectDescendants('bn-root');
    assertSetEquals(descendants, new Set(['bn-root', 'bn-c1', 'bn-c2', 'bn-gc1', 'bn-gc2', 'bn-gc3']));
});

test('collectDescendants: works from intermediate node', () => {
    setupMocks();
    // Tree: root -> parent -> child
    // When starting from 'parent', should only get parent and child
    mockNodes.set('bn-root', { id: 'bn-root', type: 'doc', doc_type: 'prd' });
    mockNodes.set('bn-parent', { id: 'bn-parent', type: 'task' });
    mockNodes.set('bn-child', { id: 'bn-child', type: 'task' });
    mockEdges = [
        { source: 'bn-parent', target: 'bn-root', edge_type: 'child_of' },
        { source: 'bn-child', target: 'bn-parent', edge_type: 'child_of' }
    ];
    
    const descendants = collectDescendants('bn-parent');
    assertSetEquals(descendants, new Set(['bn-parent', 'bn-child']));
});

console.log('\nAll tests completed!');
