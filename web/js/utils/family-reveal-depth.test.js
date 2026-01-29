/**
 * Tests for computeDepths function in family-reveal.js
 * 
 * Run with: node web/js/utils/family-reveal-depth.test.js
 */

import { reset, setEntities, setEdges } from '../state.js';
import { collectDescendants, computeDepths } from './family-reveal.js';

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

function assertMapEquals(actual, expected, message) {
    const actualEntries = Array.from(actual.entries()).sort();
    const expectedEntries = Array.from(expected.entries()).sort();
    if (JSON.stringify(actualEntries) !== JSON.stringify(expectedEntries)) {
        throw new Error(message || `Expected Map: ${JSON.stringify(expectedEntries)}, Got: ${JSON.stringify(actualEntries)}`);
    }
}

console.log('Running computeDepths tests...\n');

// Test 1: Root has depth 0
test('computeDepths: root has depth 0', () => {
    setEntities('docs', [
        { id: 'bn-root', type: 'doc', doc_type: 'prd' }
    ]);
    
    const descendants = new Set(['bn-root']);
    const depthMap = computeDepths('bn-root', descendants);
    
    assertEquals(depthMap.get('bn-root'), 0);
    assertEquals(depthMap.size, 1);
});

// Test 2: Immediate children have depth 1
test('computeDepths: immediate children have depth 1', () => {
    setEntities('docs', [
        { id: 'bn-root', type: 'doc', doc_type: 'prd' }
    ]);
    setEntities('tasks', [
        { id: 'bn-child1', type: 'task' },
        { id: 'bn-child2', type: 'task' }
    ]);
    setEdges([
        { source: 'bn-child1', target: 'bn-root', edge_type: 'child_of' },
        { source: 'bn-child2', target: 'bn-root', edge_type: 'child_of' }
    ]);
    
    const descendants = new Set(['bn-root', 'bn-child1', 'bn-child2']);
    const depthMap = computeDepths('bn-root', descendants);
    
    assertEquals(depthMap.get('bn-root'), 0);
    assertEquals(depthMap.get('bn-child1'), 1);
    assertEquals(depthMap.get('bn-child2'), 1);
});

// Test 3: Multi-level hierarchy has correct depths
test('computeDepths: multi-level hierarchy has correct depths', () => {
    setEntities('docs', [
        { id: 'bn-root', type: 'doc', doc_type: 'prd' }
    ]);
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
    
    const descendants = new Set(['bn-root', 'bn-child1', 'bn-child2', 'bn-grandchild1']);
    const depthMap = computeDepths('bn-root', descendants);
    
    assertEquals(depthMap.get('bn-root'), 0);
    assertEquals(depthMap.get('bn-child1'), 1);
    assertEquals(depthMap.get('bn-child2'), 1);
    assertEquals(depthMap.get('bn-grandchild1'), 2);
});

// Test 4: Deep tree has correct depth calculation
test('computeDepths: deep tree has correct depth calculation', () => {
    setEntities('milestones', [
        { id: 'bn-root', type: 'milestone' }
    ]);
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
    
    const descendants = new Set(['bn-root', 'bn-n1', 'bn-n2', 'bn-n3', 'bn-n4']);
    const depthMap = computeDepths('bn-root', descendants);
    
    assertEquals(depthMap.get('bn-root'), 0);
    assertEquals(depthMap.get('bn-n1'), 1);
    assertEquals(depthMap.get('bn-n2'), 2);
    assertEquals(depthMap.get('bn-n3'), 3);
    assertEquals(depthMap.get('bn-n4'), 4);
});

// Test 5: Complex tree structure has correct depths
test('computeDepths: complex tree structure has correct depths', () => {
    setEntities('docs', [
        { id: 'bn-root', type: 'doc', doc_type: 'prd' }
    ]);
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
    
    const descendants = new Set(['bn-root', 'bn-c1', 'bn-c2', 'bn-gc1', 'bn-gc2', 'bn-gc3']);
    const depthMap = computeDepths('bn-root', descendants);
    
    assertEquals(depthMap.get('bn-root'), 0);
    assertEquals(depthMap.get('bn-c1'), 1);
    assertEquals(depthMap.get('bn-c2'), 1);
    assertEquals(depthMap.get('bn-gc1'), 2);
    assertEquals(depthMap.get('bn-gc2'), 2);
    assertEquals(depthMap.get('bn-gc3'), 2);
});

// Test 6: Handles cycles without infinite loop
test('computeDepths: handles cycles without infinite loop', () => {
    setEntities('docs', [
        { id: 'bn-root', type: 'doc', doc_type: 'prd' }
    ]);
    setEntities('tasks', [
        { id: 'bn-child1', type: 'task' },
        { id: 'bn-child2', type: 'task' }
    ]);
    setEdges([
        { source: 'bn-child1', target: 'bn-root', edge_type: 'child_of' },
        { source: 'bn-child2', target: 'bn-child1', edge_type: 'child_of' },
        { source: 'bn-child1', target: 'bn-child2', edge_type: 'child_of' }
    ]);
    
    const descendants = new Set(['bn-root', 'bn-child1', 'bn-child2']);
    const depthMap = computeDepths('bn-root', descendants);
    
    assertEquals(depthMap.get('bn-root'), 0);
    assertEquals(depthMap.get('bn-child1'), 1);
    assertEquals(depthMap.get('bn-child2'), 2);
    assertEquals(depthMap.size, 3);
});

// Test 7: Only computes depth for nodes in descendants set
test('computeDepths: only computes depth for nodes in descendants set', () => {
    setEntities('docs', [
        { id: 'bn-root', type: 'doc', doc_type: 'prd' }
    ]);
    setEntities('tasks', [
        { id: 'bn-child1', type: 'task' },
        { id: 'bn-child2', type: 'task' },
        { id: 'bn-grandchild', type: 'task' }
    ]);
    setEdges([
        { source: 'bn-child1', target: 'bn-root', edge_type: 'child_of' },
        { source: 'bn-child2', target: 'bn-root', edge_type: 'child_of' },
        { source: 'bn-grandchild', target: 'bn-child1', edge_type: 'child_of' }
    ]);
    
    const descendants = new Set(['bn-root', 'bn-child1']);
    const depthMap = computeDepths('bn-root', descendants);
    
    assertEquals(depthMap.get('bn-root'), 0);
    assertEquals(depthMap.get('bn-child1'), 1);
    assertEquals(depthMap.has('bn-child2'), false);
    assertEquals(depthMap.has('bn-grandchild'), false);
    assertEquals(depthMap.size, 2);
});

// Test 8: Ignores non-child_of edges
test('computeDepths: ignores non-child_of edges', () => {
    setEntities('docs', [
        { id: 'bn-root', type: 'doc', doc_type: 'prd' }
    ]);
    setEntities('tasks', [
        { id: 'bn-child', type: 'task' },
        { id: 'bn-dep', type: 'task' }
    ]);
    setEdges([
        { source: 'bn-child', target: 'bn-root', edge_type: 'child_of' },
        { source: 'bn-dep', target: 'bn-root', edge_type: 'depends_on' }
    ]);
    
    const descendants = new Set(['bn-root', 'bn-child', 'bn-dep']);
    const depthMap = computeDepths('bn-root', descendants);
    
    assertEquals(depthMap.get('bn-root'), 0);
    assertEquals(depthMap.get('bn-child'), 1);
    assertEquals(depthMap.has('bn-dep'), false);
    assertEquals(depthMap.size, 2);
});

// Test 9: Returns empty map for empty descendants set
test('computeDepths: returns empty map for empty descendants set', () => {
    setEntities('docs', [
        { id: 'bn-root', type: 'doc', doc_type: 'prd' }
    ]);
    
    const descendants = new Set();
    const depthMap = computeDepths('bn-root', descendants);
    
    assertEquals(depthMap.size, 0);
});

// Test 10: Integration test with collectDescendants
test('computeDepths: integration with collectDescendants', () => {
    setEntities('docs', [
        { id: 'bn-prd', type: 'doc', doc_type: 'prd' }
    ]);
    setEntities('tasks', [
        { id: 'bn-t1', type: 'task' },
        { id: 'bn-t2', type: 'task' },
        { id: 'bn-t3', type: 'task' }
    ]);
    setEdges([
        { source: 'bn-t1', target: 'bn-prd', edge_type: 'child_of' },
        { source: 'bn-t2', target: 'bn-t1', edge_type: 'child_of' },
        { source: 'bn-t3', target: 'bn-t1', edge_type: 'child_of' }
    ]);
    
    const descendants = collectDescendants('bn-prd');
    const depthMap = computeDepths('bn-prd', descendants);
    
    assertEquals(descendants.size, 4);
    assertEquals(depthMap.get('bn-prd'), 0);
    assertEquals(depthMap.get('bn-t1'), 1);
    assertEquals(depthMap.get('bn-t2'), 2);
    assertEquals(depthMap.get('bn-t3'), 2);
});

console.log(`\nTests completed: ${testsPassed} passed, ${testsFailed} failed`);
if (testsFailed > 0) {
    process.exit(1);
}
