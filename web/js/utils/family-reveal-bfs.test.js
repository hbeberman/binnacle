/**
 * Simple functional tests for family-reveal.js
 * 
 * These tests use the real state module with setters for easier integration.
 * Run with: node web/js/utils/family-reveal.simple-test.js
 */

import { reset, setEntities, setEdges } from '../state.js';
import { findFamilyRoot, collectDescendants } from './family-reveal.js';

let testsPassed = 0;
let testsFailed = 0;

function test(name, fn) {
    try {
        reset(); // Reset state before each test
        fn();
        console.log(`✓ ${name}`);
        testsPassed++;
    } catch (e) {
        console.error(`✗ ${name}`);
        console.error(`  ${e.message}`);
        testsFailed++;
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

// Run tests
console.log('Running family-reveal tests...\n');

// === collectDescendants tests ===

test('collectDescendants: returns only root for node with no children', () => {
    setEntities('tasks', [{ id: 'bn-root', type: 'task' }]);
    setEdges([]);
    
    const descendants = collectDescendants('bn-root');
    assertSetEquals(descendants, new Set(['bn-root']));
});

test('collectDescendants: collects immediate children', () => {
    setEntities('docs', [{ id: 'bn-root', type: 'doc', doc_type: 'prd' }]);
    setEntities('tasks', [
        { id: 'bn-child1', type: 'task' },
        { id: 'bn-child2', type: 'task' }
    ]);
    setEdges([
        { source: 'bn-child1', target: 'bn-root', edge_type: 'child_of' },
        { source: 'bn-child2', target: 'bn-root', edge_type: 'child_of' }
    ]);
    
    const descendants = collectDescendants('bn-root');
    assertSetEquals(descendants, new Set(['bn-root', 'bn-child1', 'bn-child2']));
});

test('collectDescendants: collects multi-level hierarchy', () => {
    setEntities('docs', [{ id: 'bn-root', type: 'doc', doc_type: 'prd' }]);
    setEntities('tasks', [
        { id: 'bn-child1', type: 'task' },
        { id: 'bn-child2', type: 'task' },
        { id: 'bn-grandchild1', type: 'task' }
    ]);
    setEdges([
        { source: 'bn-child1', target: 'bn-root', edge_type: 'child_of' },
        { source: 'bn-child2', target: 'bn-root', edge_type: 'child_of' },
        { source: 'bn-grandchild1', target: 'bn-child1', edge_type: 'child_of' }
    ]);
    
    const descendants = collectDescendants('bn-root');
    assertSetEquals(descendants, new Set(['bn-root', 'bn-child1', 'bn-child2', 'bn-grandchild1']));
});

test('collectDescendants: handles deep tree', () => {
    setEntities('milestones', [{ id: 'bn-root', type: 'milestone' }]);
    setEntities('tasks', [
        { id: 'bn-n1', type: 'task' },
        { id: 'bn-n2', type: 'task' },
        { id: 'bn-n3', type: 'task' },
        { id: 'bn-n4', type: 'task' }
    ]);
    setEdges([
        { source: 'bn-n1', target: 'bn-root', edge_type: 'child_of' },
        { source: 'bn-n2', target: 'bn-n1', edge_type: 'child_of' },
        { source: 'bn-n3', target: 'bn-n2', edge_type: 'child_of' },
        { source: 'bn-n4', target: 'bn-n3', edge_type: 'child_of' }
    ]);
    
    const descendants = collectDescendants('bn-root');
    assertSetEquals(descendants, new Set(['bn-root', 'bn-n1', 'bn-n2', 'bn-n3', 'bn-n4']));
});

test('collectDescendants: handles wide tree', () => {
    setEntities('docs', [{ id: 'bn-root', type: 'doc', doc_type: 'prd' }]);
    const tasks = [];
    const edges = [];
    for (let i = 1; i <= 5; i++) {
        tasks.push({ id: `bn-child${i}`, type: 'task' });
        edges.push({ source: `bn-child${i}`, target: 'bn-root', edge_type: 'child_of' });
    }
    setEntities('tasks', tasks);
    setEdges(edges);
    
    const descendants = collectDescendants('bn-root');
    assertSetEquals(descendants, new Set(['bn-root', 'bn-child1', 'bn-child2', 'bn-child3', 'bn-child4', 'bn-child5']));
});

test('collectDescendants: prevents infinite loops with cycle detection', () => {
    setEntities('docs', [{ id: 'bn-root', type: 'doc', doc_type: 'prd' }]);
    setEntities('tasks', [
        { id: 'bn-child1', type: 'task' },
        { id: 'bn-child2', type: 'task' }
    ]);
    setEdges([
        { source: 'bn-child1', target: 'bn-root', edge_type: 'child_of' },
        { source: 'bn-child2', target: 'bn-child1', edge_type: 'child_of' },
        { source: 'bn-child1', target: 'bn-child2', edge_type: 'child_of' }  // Cycle
    ]);
    
    const descendants = collectDescendants('bn-root');
    // Should collect all nodes exactly once despite cycle
    assertSetEquals(descendants, new Set(['bn-root', 'bn-child1', 'bn-child2']));
});

test('collectDescendants: ignores non-child_of edges', () => {
    setEntities('docs', [{ id: 'bn-root', type: 'doc', doc_type: 'prd' }]);
    setEntities('tasks', [
        { id: 'bn-child', type: 'task' },
        { id: 'bn-dep', type: 'task' }
    ]);
    setEdges([
        { source: 'bn-child', target: 'bn-root', edge_type: 'child_of' },
        { source: 'bn-dep', target: 'bn-root', edge_type: 'depends_on' }  // Should be ignored
    ]);
    
    const descendants = collectDescendants('bn-root');
    assertSetEquals(descendants, new Set(['bn-root', 'bn-child']));
});

test('collectDescendants: handles complex tree structure', () => {
    setEntities('docs', [{ id: 'bn-root', type: 'doc', doc_type: 'prd' }]);
    setEntities('tasks', [
        { id: 'bn-c1', type: 'task' },
        { id: 'bn-c2', type: 'task' },
        { id: 'bn-gc1', type: 'task' },
        { id: 'bn-gc2', type: 'task' },
        { id: 'bn-gc3', type: 'task' }
    ]);
    setEdges([
        { source: 'bn-c1', target: 'bn-root', edge_type: 'child_of' },
        { source: 'bn-c2', target: 'bn-root', edge_type: 'child_of' },
        { source: 'bn-gc1', target: 'bn-c1', edge_type: 'child_of' },
        { source: 'bn-gc2', target: 'bn-c1', edge_type: 'child_of' },
        { source: 'bn-gc3', target: 'bn-c2', edge_type: 'child_of' }
    ]);
    
    const descendants = collectDescendants('bn-root');
    assertSetEquals(descendants, new Set(['bn-root', 'bn-c1', 'bn-c2', 'bn-gc1', 'bn-gc2', 'bn-gc3']));
});

test('collectDescendants: works from intermediate node', () => {
    setEntities('docs', [{ id: 'bn-root', type: 'doc', doc_type: 'prd' }]);
    setEntities('tasks', [
        { id: 'bn-parent', type: 'task' },
        { id: 'bn-child', type: 'task' }
    ]);
    setEdges([
        { source: 'bn-parent', target: 'bn-root', edge_type: 'child_of' },
        { source: 'bn-child', target: 'bn-parent', edge_type: 'child_of' }
    ]);
    
    const descendants = collectDescendants('bn-parent');
    assertSetEquals(descendants, new Set(['bn-parent', 'bn-child']));
});

// === findFamilyRoot tests ===

test('findFamilyRoot: returns PRD when clicking task with PRD ancestor', () => {
    setEntities('tasks', [{ id: 'bn-task', type: 'task' }]);
    setEntities('docs', [{ id: 'bn-prd', type: 'doc', doc_type: 'prd' }]);
    setEdges([
        { source: 'bn-task', target: 'bn-prd', edge_type: 'child_of' }
    ]);
    
    const root = findFamilyRoot('bn-task');
    assertEquals(root, 'bn-prd');
});

test('findFamilyRoot: returns null for orphan node', () => {
    setEntities('tasks', [{ id: 'bn-orphan', type: 'task' }]);
    setEdges([]);
    
    const root = findFamilyRoot('bn-orphan');
    assertNull(root);
});

test('findFamilyRoot: handles deep hierarchy', () => {
    setEntities('tasks', [
        { id: 'bn-task1', type: 'task' },
        { id: 'bn-task2', type: 'task' },
        { id: 'bn-task3', type: 'task' }
    ]);
    setEntities('docs', [{ id: 'bn-prd', type: 'doc', doc_type: 'prd' }]);
    setEdges([
        { source: 'bn-task1', target: 'bn-task2', edge_type: 'child_of' },
        { source: 'bn-task2', target: 'bn-task3', edge_type: 'child_of' },
        { source: 'bn-task3', target: 'bn-prd', edge_type: 'child_of' }
    ]);
    
    const root = findFamilyRoot('bn-task1');
    assertEquals(root, 'bn-prd');
});

// Print summary
console.log(`\n=== Test Summary ===`);
console.log(`Passed: ${testsPassed}`);
console.log(`Failed: ${testsFailed}`);
console.log(`Total: ${testsPassed + testsFailed}`);

// Exit with error code if any tests failed (Node.js only)
// eslint-disable-next-line no-undef
if (typeof process !== 'undefined' && testsFailed > 0) {
    // eslint-disable-next-line no-undef
    process.exit(1);
}
