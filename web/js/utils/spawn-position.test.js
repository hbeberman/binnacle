/**
 * Tests for spawn position calculation
 */

import { computeSpawnPositions } from './spawn-position.js';

// Mock state for testing
let mockEdges = [];

// Mock state module
globalThis.state = {
    get: (key) => {
        if (key === 'edges') {
            return mockEdges;
        }
        return null;
    }
};

// Import getEdges after setting up mock
const { getEdges } = await import('../state.js');

let testsPassed = 0;
let testsFailed = 0;

function test(name, fn) {
    try {
        fn();
        console.log(`✓ ${name}`);
        testsPassed++;
    } catch (error) {
        console.error(`✗ ${name}`);
        console.error(error);
        testsFailed++;
    }
}

function assertEquals(actual, expected, message) {
    if (actual !== expected) {
        throw new Error(message || `Expected: ${expected}, Got: ${actual}`);
    }
}

function assertApproxEquals(actual, expected, tolerance, message) {
    if (Math.abs(actual - expected) > tolerance) {
        throw new Error(message || `Expected: ${expected} ± ${tolerance}, Got: ${actual}`);
    }
}

function assertPositionValid(position, message) {
    if (!position || typeof position.x !== 'number' || typeof position.y !== 'number') {
        throw new Error(message || `Invalid position: ${JSON.stringify(position)}`);
    }
}

// Reset test environment
function reset() {
    mockEdges = [];
}

// === Root node tests ===

test('single root node gets position near origin', () => {
    reset();
    const depthMap = new Map([['bn-root', 0]]);
    const existingNodes = new Map();
    mockEdges = [];
    
    const positions = computeSpawnPositions(depthMap, existingNodes, mockEdges);
    
    assertEquals(positions.size, 1);
    const pos = positions.get('bn-root');
    assertPositionValid(pos);
    // Should be near origin (within 20 units due to randomization)
    assertApproxEquals(pos.x, 0, 20, 'Root X should be near origin');
    assertApproxEquals(pos.y, 0, 20, 'Root Y should be near origin');
});

test('root node with existing position uses that position', () => {
    reset();
    const depthMap = new Map([['bn-root', 0]]);
    const existingNodes = new Map([
        ['bn-root', { id: 'bn-root', x: 100, y: 200 }]
    ]);
    mockEdges = [];
    
    const positions = computeSpawnPositions(depthMap, existingNodes, mockEdges);
    
    const pos = positions.get('bn-root');
    assertEquals(pos.x, 100);
    assertEquals(pos.y, 200);
});

// === Parent-child positioning tests ===

test('child node positioned below parent', () => {
    reset();
    const depthMap = new Map([
        ['bn-root', 0],
        ['bn-child', 1]
    ]);
    const existingNodes = new Map([
        ['bn-root', { id: 'bn-root', x: 0, y: 0 }]
    ]);
    mockEdges = [
        { source: 'bn-child', target: 'bn-root', edge_type: 'child_of' }
    ];
    
    const positions = computeSpawnPositions(depthMap, existingNodes, mockEdges);
    
    const rootPos = positions.get('bn-root');
    const childPos = positions.get('bn-child');
    
    assertEquals(rootPos.x, 0);
    assertEquals(rootPos.y, 0);
    assertPositionValid(childPos);
    
    // Child should be below parent (positive Y direction)
    if (childPos.y <= rootPos.y) {
        throw new Error(`Child Y (${childPos.y}) should be greater than parent Y (${rootPos.y})`);
    }
});

test('multiple children spread horizontally', () => {
    reset();
    const depthMap = new Map([
        ['bn-root', 0],
        ['bn-child1', 1],
        ['bn-child2', 1],
        ['bn-child3', 1]
    ]);
    const existingNodes = new Map([
        ['bn-root', { id: 'bn-root', x: 0, y: 0 }]
    ]);
    mockEdges = [
        { source: 'bn-child1', target: 'bn-root', edge_type: 'child_of' },
        { source: 'bn-child2', target: 'bn-root', edge_type: 'child_of' },
        { source: 'bn-child3', target: 'bn-root', edge_type: 'child_of' }
    ];
    
    
    const positions = computeSpawnPositions(depthMap, existingNodes, mockEdges);
    
    
    const child1Pos = positions.get('bn-child1');
    const child2Pos = positions.get('bn-child2');
    const child3Pos = positions.get('bn-child3');
    
    
    assertPositionValid(child1Pos);
    assertPositionValid(child2Pos);
    assertPositionValid(child3Pos);
    
    // Children should have different X positions (spread horizontally)
    const xPositions = [child1Pos.x, child2Pos.x, child3Pos.x].sort((a, b) => a - b);
    if (Math.abs(xPositions[0] - xPositions[1]) < 10 || Math.abs(xPositions[1] - xPositions[2]) < 10) {
        throw new Error('Children should be spread horizontally with significant spacing');
    }
});

test('existing child node keeps its position', () => {
    reset();
    const depthMap = new Map([
        ['bn-root', 0],
        ['bn-child', 1]
    ]);
    const existingNodes = new Map([
        ['bn-root', { id: 'bn-root', x: 0, y: 0 }],
        ['bn-child', { id: 'bn-child', x: 50, y: 150 }]
    ]);
    mockEdges = [
        { source: 'bn-child', target: 'bn-root', edge_type: 'child_of' }
    ];
    
    const positions = computeSpawnPositions(depthMap, existingNodes, mockEdges);
    
    const childPos = positions.get('bn-child');
    assertEquals(childPos.x, 50);
    assertEquals(childPos.y, 150);
});

// === Multi-level hierarchy tests ===

test('grandchildren positioned relative to parents at correct depth', () => {
    reset();
    const depthMap = new Map([
        ['bn-root', 0],
        ['bn-child', 1],
        ['bn-grandchild', 2]
    ]);
    const existingNodes = new Map([
        ['bn-root', { id: 'bn-root', x: 0, y: 0 }]
    ]);
    mockEdges = [
        { source: 'bn-child', target: 'bn-root', edge_type: 'child_of' },
        { source: 'bn-grandchild', target: 'bn-child', edge_type: 'child_of' }
    ];
    
    const positions = computeSpawnPositions(depthMap, existingNodes, mockEdges);
    
    const rootPos = positions.get('bn-root');
    const childPos = positions.get('bn-child');
    const grandchildPos = positions.get('bn-grandchild');
    
    assertPositionValid(childPos);
    assertPositionValid(grandchildPos);
    
    // Child should be below root
    if (childPos.y <= rootPos.y) {
        throw new Error('Child should be below root');
    }
    
    // Grandchild should be below child
    if (grandchildPos.y <= childPos.y) {
        throw new Error('Grandchild should be below child');
    }
});

test('deep hierarchy maintains correct spacing', () => {
    reset();
    const depthMap = new Map([
        ['bn-root', 0],
        ['bn-n1', 1],
        ['bn-n2', 2],
        ['bn-n3', 3]
    ]);
    const existingNodes = new Map([
        ['bn-root', { id: 'bn-root', x: 0, y: 0 }]
    ]);
    mockEdges = [
        { source: 'bn-n1', target: 'bn-root', edge_type: 'child_of' },
        { source: 'bn-n2', target: 'bn-n1', edge_type: 'child_of' },
        { source: 'bn-n3', target: 'bn-n2', edge_type: 'child_of' }
    ];
    
    const positions = computeSpawnPositions(depthMap, existingNodes, mockEdges);
    
    const rootPos = positions.get('bn-root');
    const n1Pos = positions.get('bn-n1');
    const n2Pos = positions.get('bn-n2');
    const n3Pos = positions.get('bn-n3');
    
    // Each level should be further down
    if (!(rootPos.y < n1Pos.y && n1Pos.y < n2Pos.y && n2Pos.y < n3Pos.y)) {
        throw new Error('Deep hierarchy should have increasing Y coordinates');
    }
    
    // Distance should increase with depth (baseDistance = 100 + depth * 20)
    const dist1 = Math.sqrt((n1Pos.x - rootPos.x) ** 2 + (n1Pos.y - rootPos.y) ** 2);
    const dist2 = Math.sqrt((n2Pos.x - n1Pos.x) ** 2 + (n2Pos.y - n1Pos.y) ** 2);
    const dist3 = Math.sqrt((n3Pos.x - n2Pos.x) ** 2 + (n3Pos.y - n2Pos.y) ** 2);
    
    // Distances should roughly increase (accounting for randomization)
    // dist1 ≈ 100, dist2 ≈ 120, dist3 ≈ 140
    assertApproxEquals(dist1, 100, 30, 'Depth 1 distance should be ~100');
    assertApproxEquals(dist2, 120, 30, 'Depth 2 distance should be ~120');
    assertApproxEquals(dist3, 140, 30, 'Depth 3 distance should be ~140');
});

// === Complex tree structure tests ===

test('wide tree with multiple children at each level', () => {
    reset();
    const depthMap = new Map([
        ['bn-root', 0],
        ['bn-c1', 1],
        ['bn-c2', 1],
        ['bn-gc1', 2],
        ['bn-gc2', 2]
    ]);
    const existingNodes = new Map([
        ['bn-root', { id: 'bn-root', x: 0, y: 0 }]
    ]);
    mockEdges = [
        { source: 'bn-c1', target: 'bn-root', edge_type: 'child_of' },
        { source: 'bn-c2', target: 'bn-root', edge_type: 'child_of' },
        { source: 'bn-gc1', target: 'bn-c1', edge_type: 'child_of' },
        { source: 'bn-gc2', target: 'bn-c2', edge_type: 'child_of' }
    ];
    
    const positions = computeSpawnPositions(depthMap, existingNodes, mockEdges);
    
    assertEquals(positions.size, 5);
    
    const c1Pos = positions.get('bn-c1');
    const c2Pos = positions.get('bn-c2');
    const gc1Pos = positions.get('bn-gc1');
    const gc2Pos = positions.get('bn-gc2');
    
    assertPositionValid(c1Pos);
    assertPositionValid(c2Pos);
    assertPositionValid(gc1Pos);
    assertPositionValid(gc2Pos);
    
    // Children should be spread horizontally
    if (Math.abs(c1Pos.x - c2Pos.x) < 10) {
        throw new Error('Children should be horizontally separated');
    }
});

test('empty depth map returns empty positions', () => {
    reset();
    const depthMap = new Map();
    const existingNodes = new Map();
    mockEdges = [];
    
    const positions = computeSpawnPositions(depthMap, existingNodes, mockEdges);
    
    assertEquals(positions.size, 0);
});

test('nodes with non-child_of edges are not affected', () => {
    reset();
    const depthMap = new Map([
        ['bn-root', 0],
        ['bn-child', 1]
    ]);
    const existingNodes = new Map([
        ['bn-root', { id: 'bn-root', x: 0, y: 0 }]
    ]);
    // Use depends_on instead of child_of
    mockEdges = [
        { source: 'bn-child', target: 'bn-root', edge_type: 'depends_on' }
    ];
    
    const positions = computeSpawnPositions(depthMap, existingNodes, mockEdges);
    
    const childPos = positions.get('bn-child');
    assertPositionValid(childPos);
    
    // Child should still get positioned, but near origin since no parent found
    assertApproxEquals(childPos.x, 0, 20, 'Child without parent should be near origin');
    assertApproxEquals(childPos.y, 0, 20, 'Child without parent should be near origin');
});

// Print summary
console.log(`\n=== Test Summary ===`);
console.log(`Passed: ${testsPassed}`);
console.log(`Failed: ${testsFailed}`);
console.log(`Total: ${testsPassed + testsFailed}`);

// Exit with error code if any tests failed (Node.js only)
if (typeof process !== 'undefined' && testsFailed > 0) {
    process.exit(1);
}
