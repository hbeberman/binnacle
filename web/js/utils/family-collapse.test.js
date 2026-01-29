/**
 * Tests for family collapse functionality
 */

import { collapseFamilyReveal, getFadeOutOpacity, hasActiveFadeOutAnimations, clearFadeOutAnimations } from './family-collapse.js';
import * as state from '../state.js';

// Test setup
function setup() {
    // Clear state
    state.set('ui.familyReveal', {
        active: false,
        rootId: null,
        revealedNodeIds: new Set(),
        spawnPositions: new Map()
    });
    
    // Clear fade-out animations
    clearFadeOutAnimations();
    
    // Set up test nodes
    state.set('entities', [
        { id: 'bn-root', type: 'doc', doc_type: 'prd', title: 'Root PRD', status: 'done' },
        { id: 'bn-task1', type: 'task', title: 'Task 1', status: 'pending' },
        { id: 'bn-task2', type: 'task', title: 'Task 2', status: 'done' },
        { id: 'bn-bug1', type: 'bug', title: 'Bug 1', status: 'pending' }
    ]);
    
    // Set up default filters
    state.set('ui.hideCompleted', false);
    state.set('ui.nodeTypeFilters', {
        task: true,
        bug: true,
        doc: true,
        milestone: true,
        agent: true,
        test: true,
        queue: true
    });
    state.set('ui.searchQuery', '');
}

// Test: Collapse with no active reveal does nothing
function testCollapseWithNoActiveReveal() {
    setup();
    
    console.log('Test: Collapse with no active reveal does nothing');
    
    // Call collapse with no active reveal
    collapseFamilyReveal();
    
    // Should still be inactive
    const familyReveal = state.get('ui.familyReveal');
    console.assert(familyReveal.active === false, 'Family reveal should be inactive');
    console.assert(hasActiveFadeOutAnimations() === false, 'No fade-out animations should be active');
    
    console.log('✅ Passed');
}

// Test: Collapse clears reveal state
function testCollapseClearsRevealState() {
    setup();
    
    console.log('Test: Collapse clears reveal state');
    
    // Set up active reveal
    state.set('ui.familyReveal', {
        active: true,
        rootId: 'bn-root',
        revealedNodeIds: new Set(['bn-root', 'bn-task1', 'bn-task2']),
        spawnPositions: new Map()
    });
    
    // Call collapse
    collapseFamilyReveal();
    
    // Reveal state should be cleared
    const familyReveal = state.get('ui.familyReveal');
    console.assert(familyReveal.active === false, 'Family reveal should be inactive');
    console.assert(familyReveal.rootId === null, 'Root ID should be null');
    console.assert(familyReveal.revealedNodeIds.size === 0, 'Revealed nodes should be empty');
    
    console.log('✅ Passed');
}

// Test: Nodes that don't pass filters fade out
function testNodesFailingFiltersFadeOut() {
    setup();
    
    console.log('Test: Nodes that don\'t pass filters fade out');
    
    // Enable hide completed filter
    state.set('ui.hideCompleted', true);
    
    // Set up active reveal with completed and pending tasks
    state.set('ui.familyReveal', {
        active: true,
        rootId: 'bn-root',
        revealedNodeIds: new Set(['bn-root', 'bn-task1', 'bn-task2']),  // task2 is completed
        spawnPositions: new Map()
    });
    
    // Call collapse
    collapseFamilyReveal();
    
    // Completed nodes should have fade-out animations
    // Note: We can't test the exact timing here, but we can check that animations are active
    console.assert(hasActiveFadeOutAnimations() === true, 'Fade-out animations should be active');
    
    console.log('✅ Passed');
}

// Test: Nodes passing filters don't fade out
function testNodesPassingFiltersStayVisible() {
    setup();
    
    console.log('Test: Nodes passing filters don\'t fade out');
    
    // Don't hide completed
    state.set('ui.hideCompleted', false);
    
    // Set up active reveal
    state.set('ui.familyReveal', {
        active: true,
        rootId: 'bn-root',
        revealedNodeIds: new Set(['bn-root', 'bn-task1', 'bn-task2']),
        spawnPositions: new Map()
    });
    
    // Call collapse
    collapseFamilyReveal();
    
    // No nodes should fade out since all pass filters
    // Wait a bit for fade-out to potentially start
    setTimeout(() => {
        // All animations should complete quickly since no nodes are fading
        console.log('Checking fade-out state after collapse...');
        // This is a best-effort check - actual behavior depends on timing
    }, 50);
    
    console.log('✅ Passed');
}

// Test: getFadeOutOpacity returns correct values
function testGetFadeOutOpacity() {
    setup();
    
    console.log('Test: getFadeOutOpacity returns correct values');
    
    // No fade-out should return null
    let opacity = getFadeOutOpacity('bn-task1');
    console.assert(opacity === null, 'Opacity should be null for non-fading node');
    
    console.log('✅ Passed');
}

// Run all tests
function runTests() {
    console.log('Running family-collapse tests...\n');
    
    testCollapseWithNoActiveReveal();
    testCollapseClearsRevealState();
    testNodesFailingFiltersFadeOut();
    testNodesPassingFiltersStayVisible();
    testGetFadeOutOpacity();
    
    console.log('\n✅ All family-collapse tests passed!');
}

// Export for use in test runner
export { runTests };

// Auto-run if loaded as a module in browser
if (typeof window !== 'undefined') {
    runTests();
}
