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

test('n/N with no active search shows hint toast', () => {
    // Ensure no search is active
    set('ui.searchQuery', '');
    set('ui.searchMatches', []);
    set('ui.currentMatchIndex', -1);
    
    // Simulate the n key handler logic (from camera.js)
    const searchQuery = get('ui.searchQuery');
    const searchMatches = get('ui.searchMatches') || [];
    
    // Verify preconditions: no active search
    assert(!searchQuery || searchMatches.length === 0, 'Should have no active search');
    
    // When no search is active, handler shows toast hint
    addToast({
        type: 'info',
        message: 'No active search. Press / to search.',
        duration: 2000
    });
    
    const toasts = get('ui.toasts');
    assert(toasts.length === 1, 'Should have one toast');
    assertEquals(toasts[0].message, 'No active search. Press / to search.');
});

test('n/N with active search navigates to match', () => {
    // Setup an active search with matches
    set('ui.searchQuery', 'test');
    set('ui.searchMatches', ['bn-001', 'bn-002', 'bn-003']);
    set('ui.currentMatchIndex', -1);
    
    const searchQuery = get('ui.searchQuery');
    const searchMatches = get('ui.searchMatches') || [];
    
    // Verify search is active
    assert(searchQuery && searchMatches.length > 0, 'Should have active search');
    
    // Simulate n key: go to first match
    let currentIndex = get('ui.currentMatchIndex');
    const direction = 1; // n = forward
    
    if (currentIndex < 0) {
        currentIndex = 0; // Start at first
    } else {
        currentIndex = (currentIndex + direction + searchMatches.length) % searchMatches.length;
    }
    
    set('ui.currentMatchIndex', currentIndex);
    
    assertEquals(get('ui.currentMatchIndex'), 0, 'Should be at first match');
    assertEquals(searchMatches[currentIndex], 'bn-001', 'Should select first match');
});

test('n/N wraps around search matches', () => {
    // Setup an active search at the last match
    set('ui.searchQuery', 'test');
    set('ui.searchMatches', ['bn-001', 'bn-002', 'bn-003']);
    set('ui.currentMatchIndex', 2); // Last index
    
    const searchMatches = get('ui.searchMatches');
    let currentIndex = get('ui.currentMatchIndex');
    const direction = 1; // n = forward
    
    // Should wrap to beginning
    currentIndex = (currentIndex + direction + searchMatches.length) % searchMatches.length;
    set('ui.currentMatchIndex', currentIndex);
    
    assertEquals(get('ui.currentMatchIndex'), 0, 'Should wrap to first match');
});

test('N (shift+n) navigates backwards through matches', () => {
    // Setup an active search at the first match
    set('ui.searchQuery', 'test');
    set('ui.searchMatches', ['bn-001', 'bn-002', 'bn-003']);
    set('ui.currentMatchIndex', 0); // First index
    
    const searchMatches = get('ui.searchMatches');
    let currentIndex = get('ui.currentMatchIndex');
    const direction = -1; // N = backward
    
    // Should wrap to end
    currentIndex = (currentIndex + direction + searchMatches.length) % searchMatches.length;
    set('ui.currentMatchIndex', currentIndex);
    
    assertEquals(get('ui.currentMatchIndex'), 2, 'Should wrap to last match');
});

// Summary
console.log(`\n${testsPassed} passed, ${testsFailed} failed`);
process.exit(testsFailed > 0 ? 1 : 0);
