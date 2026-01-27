/**
 * Tests for multi-selection state management
 * 
 * Run with: node web/js/state-multiselect.test.js
 */

import {
    reset,
    getSelectedNodes,
    isSelected,
    toggleSelection,
    setSelectedNodes,
    clearSelection,
    selectAll,
    getSelectedNode,
    setSelectedNode
} from './state.js';

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

function assert(condition, message) {
    if (!condition) {
        throw new Error(message || 'Assertion failed');
    }
}

function assertEquals(actual, expected, message) {
    if (JSON.stringify(actual) !== JSON.stringify(expected)) {
        throw new Error(message || `Expected ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`);
    }
}

// Test suite
console.log('Running multi-selection state tests...\n');

test('Initial state has empty selectedNodes array', () => {
    const nodes = getSelectedNodes();
    assertEquals(nodes, [], 'selectedNodes should be empty array');
});

test('isSelected returns false for unselected node', () => {
    assert(!isSelected('bn-test1'), 'Node should not be selected');
});

test('toggleSelection with clearOthers adds single node', () => {
    toggleSelection('bn-test1', true);
    assertEquals(getSelectedNodes(), ['bn-test1']);
    assert(isSelected('bn-test1'), 'Node should be selected');
});

test('toggleSelection without clearOthers adds to selection', () => {
    toggleSelection('bn-test1', true);
    toggleSelection('bn-test2', false);
    assertEquals(getSelectedNodes(), ['bn-test1', 'bn-test2']);
    assert(isSelected('bn-test1'), 'First node should be selected');
    assert(isSelected('bn-test2'), 'Second node should be selected');
});

test('toggleSelection removes already selected node', () => {
    toggleSelection('bn-test1', true);
    toggleSelection('bn-test2', false);
    toggleSelection('bn-test1', false); // Toggle off
    assertEquals(getSelectedNodes(), ['bn-test2']);
    assert(!isSelected('bn-test1'), 'First node should be deselected');
    assert(isSelected('bn-test2'), 'Second node should still be selected');
});

test('setSelectedNodes replaces current selection', () => {
    toggleSelection('bn-test1', true);
    setSelectedNodes(['bn-test2', 'bn-test3']);
    assertEquals(getSelectedNodes(), ['bn-test2', 'bn-test3']);
    assert(!isSelected('bn-test1'), 'First node should not be selected');
    assert(isSelected('bn-test2'), 'Second node should be selected');
    assert(isSelected('bn-test3'), 'Third node should be selected');
});

test('clearSelection removes all selections', () => {
    setSelectedNodes(['bn-test1', 'bn-test2', 'bn-test3']);
    clearSelection();
    assertEquals(getSelectedNodes(), []);
    assert(!isSelected('bn-test1'), 'No nodes should be selected');
});

test('selectAll selects all provided nodes', () => {
    const visibleNodes = [
        { id: 'bn-test1' },
        { id: 'bn-test2' },
        { id: 'bn-test3' }
    ];
    selectAll(visibleNodes);
    assertEquals(getSelectedNodes(), ['bn-test1', 'bn-test2', 'bn-test3']);
});

test('Backward compatibility: selectedNode updated when single node selected', () => {
    toggleSelection('bn-test1', true);
    assertEquals(getSelectedNode(), 'bn-test1');
});

test('Backward compatibility: selectedNode is last when multiple selected', () => {
    setSelectedNodes(['bn-test1', 'bn-test2', 'bn-test3']);
    assertEquals(getSelectedNode(), 'bn-test3');
});

test('Backward compatibility: selectedNode is null when cleared', () => {
    setSelectedNodes(['bn-test1', 'bn-test2']);
    clearSelection();
    assertEquals(getSelectedNode(), null);
});

test('Backward compatibility: setSelectedNode updates both fields', () => {
    setSelectedNode('bn-test1');
    assertEquals(getSelectedNode(), 'bn-test1');
    assertEquals(getSelectedNodes(), ['bn-test1']);
});

test('Backward compatibility: setSelectedNode(null) clears both fields', () => {
    setSelectedNode('bn-test1');
    setSelectedNode(null);
    assertEquals(getSelectedNode(), null);
    assertEquals(getSelectedNodes(), []);
});

// Summary
console.log(`\n${testsPassed} passed, ${testsFailed} failed`);
process.exit(testsFailed > 0 ? 1 : 0);
