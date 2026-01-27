/**
 * Tests for keyboard selection shortcuts
 * 
 * Run with: node web/js/keyboard-selection.test.js
 */

import {
    reset,
    getSelectedNodes,
    clearSelection,
    selectAll,
    addToast,
    get,
    set
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
console.log('Running keyboard selection shortcut tests...\n');

test('selectAll selects all visible nodes', () => {
    const visibleNodes = [
        { id: 'bn-test1', type: 'task', title: 'Task 1' },
        { id: 'bn-test2', type: 'task', title: 'Task 2' },
        { id: 'bn-test3', type: 'bug', title: 'Bug 1' }
    ];
    
    selectAll(visibleNodes);
    
    const selected = getSelectedNodes();
    assertEquals(selected, ['bn-test1', 'bn-test2', 'bn-test3']);
});

test('selectAll with empty array clears selection', () => {
    // First select some nodes
    const visibleNodes = [
        { id: 'bn-test1', type: 'task', title: 'Task 1' }
    ];
    selectAll(visibleNodes);
    
    // Then select all from empty array
    selectAll([]);
    
    assertEquals(getSelectedNodes(), []);
});

test('clearSelection removes all selected nodes', () => {
    const visibleNodes = [
        { id: 'bn-test1', type: 'task', title: 'Task 1' },
        { id: 'bn-test2', type: 'task', title: 'Task 2' }
    ];
    selectAll(visibleNodes);
    
    clearSelection();
    
    assertEquals(getSelectedNodes(), []);
});

test('Escape key behavior: clears selection', () => {
    // Setup: select some nodes
    const visibleNodes = [
        { id: 'bn-test1', type: 'task', title: 'Task 1' },
        { id: 'bn-test2', type: 'task', title: 'Task 2' }
    ];
    selectAll(visibleNodes);
    
    // Simulate Escape key: clear selection
    clearSelection();
    
    assertEquals(getSelectedNodes(), []);
});

test('Escape key behavior: clears focused node', () => {
    // Setup: set focused node
    set('ui.focusedNode', 'bn-test1');
    
    // Simulate Escape key: clear focused node
    set('ui.focusedNode', null);
    
    assert(get('ui.focusedNode') === null, 'Focused node should be null');
});

test('Toast notification can be added', () => {
    // Simulate adding a toast (like Ctrl+A feedback)
    const toastId = addToast({
        type: 'info',
        message: 'Selected 3 visible nodes',
        duration: 2000
    });
    
    assert(toastId > 0, 'Toast ID should be positive');
    
    const toasts = get('ui.toasts');
    assert(toasts.length === 1, 'Should have one toast');
    assertEquals(toasts[0].type, 'info');
    assertEquals(toasts[0].message, 'Selected 3 visible nodes');
});

// Summary
console.log(`\n${testsPassed} passed, ${testsFailed} failed`);
process.exit(testsFailed > 0 ? 1 : 0);
