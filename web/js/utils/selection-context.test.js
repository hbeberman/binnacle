/**
 * Tests for selection context gathering
 * 
 * Run with: node web/js/utils/selection-context.test.js
 */

import {
    reset,
    setEntities,
    setEdges,
    set,
    setSelectedNodes
} from '../state.js';

import {
    gatherSelectionContext,
    formatContextAsMarkdown,
    formatContextAsJSON
} from './selection-context.js';

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
        if (e.stack) {
            console.error('  Stack:', e.stack.split('\n').slice(1, 3).join('\n  '));
        }
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
        throw new Error(
            message || 
            `Expected ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`
        );
    }
}

// Test suite
console.log('Running selection context tests...\n');

test('Returns empty context when nothing selected', () => {
    const context = gatherSelectionContext();
    assertEquals(context.selectionCount, 0);
    assertEquals(context.entities, []);
    assertEquals(context.internalEdges, []);
    assertEquals(context.externalEdges, []);
    assert(context.summary.includes('No entities selected'));
});

test('Gathers full entity data for selected nodes', () => {
    // Setup test data
    setEntities('tasks', [
        { id: 'bn-test1', type: 'task', title: 'Task 1', status: 'pending', priority: 1 },
        { id: 'bn-test2', type: 'task', title: 'Task 2', status: 'done', priority: 2 }
    ]);
    
    setSelectedNodes(['bn-test1', 'bn-test2']);
    
    const context = gatherSelectionContext();
    
    assertEquals(context.selectionCount, 2);
    assertEquals(context.entities.length, 2);
    assert(context.entities[0].id === 'bn-test1');
    assert(context.entities[1].id === 'bn-test2');
    assert(context.entities[0].title === 'Task 1');
    assert(context.entities[1].title === 'Task 2');
});

test('Identifies internal edges between selected nodes', () => {
    setEntities('tasks', [
        { id: 'bn-test1', type: 'task', title: 'Task 1' },
        { id: 'bn-test2', type: 'task', title: 'Task 2' },
        { id: 'bn-test3', type: 'task', title: 'Task 3' }
    ]);
    
    setEdges([
        { source: 'bn-test1', target: 'bn-test2', type: 'depends_on' },
        { source: 'bn-test2', target: 'bn-test3', type: 'blocks' }
    ]);
    
    setSelectedNodes(['bn-test1', 'bn-test2']);
    
    const context = gatherSelectionContext();
    
    assertEquals(context.internalEdges.length, 1);
    assert(context.internalEdges[0].source === 'bn-test1');
    assert(context.internalEdges[0].target === 'bn-test2');
    assert(context.internalEdges[0].type === 'depends_on');
});

test('Identifies external edges to non-selected nodes', () => {
    setEntities('tasks', [
        { id: 'bn-test1', type: 'task', title: 'Task 1', status: 'pending' },
        { id: 'bn-test2', type: 'task', title: 'Task 2', status: 'done' },
        { id: 'bn-test3', type: 'task', title: 'Task 3', status: 'pending' }
    ]);
    
    setEdges([
        { source: 'bn-test1', target: 'bn-test3', type: 'depends_on' },
        { source: 'bn-test3', target: 'bn-test2', type: 'blocks' }
    ]);
    
    setSelectedNodes(['bn-test1', 'bn-test2']);
    
    const context = gatherSelectionContext();
    
    assertEquals(context.externalEdges.length, 2);
    
    // Find outbound edge
    const outbound = context.externalEdges.find(e => e.direction === 'outbound');
    assert(outbound.selectedNode === 'bn-test1');
    assert(outbound.externalNode === 'bn-test3');
    assert(outbound.direction === 'outbound');
    
    // Find inbound edge
    const inbound = context.externalEdges.find(e => e.direction === 'inbound');
    assert(inbound.selectedNode === 'bn-test2');
    assert(inbound.externalNode === 'bn-test3');
    assert(inbound.direction === 'inbound');
});

test('Can exclude external edges with option', () => {
    setEntities('tasks', [
        { id: 'bn-test1', type: 'task', title: 'Task 1' },
        { id: 'bn-test2', type: 'task', title: 'Task 2' }
    ]);
    
    setEdges([
        { source: 'bn-test1', target: 'bn-test2', type: 'depends_on' }
    ]);
    
    setSelectedNodes(['bn-test1']);
    
    const context = gatherSelectionContext({ includeExternalEdges: false });
    
    assertEquals(context.externalEdges.length, 0);
});

test('Gathers recent activity for selected entities', () => {
    setEntities('tasks', [
        { id: 'bn-test1', type: 'task', title: 'Task 1' }
    ]);
    
    set('log', [
        {
            timestamp: '2026-01-27T10:00:00Z',
            entity_id: 'bn-test1',
            action: 'update',
            field: 'status',
            old_value: 'pending',
            new_value: 'done',
            actor: 'agent-123'
        },
        {
            timestamp: '2026-01-27T09:00:00Z',
            entity_id: 'bn-test2',
            action: 'create',
            actor: 'user1'
        },
        {
            timestamp: '2026-01-27T08:00:00Z',
            entity_id: 'bn-test1',
            action: 'create',
            actor: 'user1'
        }
    ]);
    
    setSelectedNodes(['bn-test1']);
    
    const context = gatherSelectionContext();
    
    assertEquals(context.recentActivity.length, 2);
    // Should be sorted by timestamp, most recent first
    assert(context.recentActivity[0].timestamp === '2026-01-27T10:00:00Z');
    assert(context.recentActivity[1].timestamp === '2026-01-27T08:00:00Z');
    assert(context.recentActivity[0].action === 'update');
});

test('Limits log entries with maxLogEntries option', () => {
    setEntities('tasks', [
        { id: 'bn-test1', type: 'task', title: 'Task 1' }
    ]);
    
    const logEntries = [];
    for (let i = 0; i < 20; i++) {
        logEntries.push({
            timestamp: `2026-01-27T${String(i).padStart(2, '0')}:00:00Z`,
            entity_id: 'bn-test1',
            action: 'update',
            field: 'status'
        });
    }
    set('log', logEntries);
    
    setSelectedNodes(['bn-test1']);
    
    const context = gatherSelectionContext({ maxLogEntries: 5 });
    
    assert(context.recentActivity.length <= 5);
});

test('Generates accurate summary', () => {
    setEntities('tasks', [
        { id: 'bn-test1', type: 'task', title: 'Task 1' },
        { id: 'bn-test2', type: 'task', title: 'Task 2' }
    ]);
    setEntities('bugs', [
        { id: 'bn-bug1', type: 'bug', title: 'Bug 1' }
    ]);
    
    setEdges([
        { source: 'bn-test1', target: 'bn-test2', type: 'depends_on' }
    ]);
    
    setSelectedNodes(['bn-test1', 'bn-test2', 'bn-bug1']);
    
    const context = gatherSelectionContext();
    
    assert(context.summary.includes('3 entities'));
    assert(context.summary.includes('2 tasks'));
    assert(context.summary.includes('1 bug'));
    assert(context.summary.includes('1 internal relationship'));
});

test('Metadata includes entity type counts', () => {
    setEntities('tasks', [
        { id: 'bn-test1', type: 'task', title: 'Task 1' },
        { id: 'bn-test2', type: 'task', title: 'Task 2' }
    ]);
    setEntities('ideas', [
        { id: 'bn-idea1', type: 'idea', title: 'Idea 1' }
    ]);
    
    setSelectedNodes(['bn-test1', 'bn-test2', 'bn-idea1']);
    
    const context = gatherSelectionContext();
    
    assertEquals(context.metadata.entityTypes.task, 2);
    assertEquals(context.metadata.entityTypes.idea, 1);
    assert(context.metadata.timestamp);
});

test('formatContextAsMarkdown produces valid markdown', () => {
    setEntities('tasks', [
        { 
            id: 'bn-test1', 
            type: 'task', 
            title: 'Task 1',
            description: 'Test description',
            status: 'pending',
            priority: 1,
            tags: ['backend', 'api']
        }
    ]);
    
    setSelectedNodes(['bn-test1']);
    
    const context = gatherSelectionContext();
    const markdown = formatContextAsMarkdown(context);
    
    assert(markdown.includes('# Selection Context'));
    assert(markdown.includes('## Selected Entities'));
    assert(markdown.includes('bn-test1: Task 1'));
    assert(markdown.includes('**Type:** task'));
    assert(markdown.includes('**Status:** pending'));
    assert(markdown.includes('**Priority:** 1'));
    assert(markdown.includes('**Description:** Test description'));
    assert(markdown.includes('**Tags:** backend, api'));
});

test('formatContextAsMarkdown handles empty selection', () => {
    const context = gatherSelectionContext();
    const markdown = formatContextAsMarkdown(context);
    
    assert(markdown.includes('No entities selected'));
});

test('formatContextAsMarkdown includes internal edges', () => {
    setEntities('tasks', [
        { id: 'bn-test1', type: 'task', title: 'Task 1' },
        { id: 'bn-test2', type: 'task', title: 'Task 2' }
    ]);
    
    setEdges([
        { source: 'bn-test1', target: 'bn-test2', type: 'depends_on' }
    ]);
    
    setSelectedNodes(['bn-test1', 'bn-test2']);
    
    const context = gatherSelectionContext();
    const markdown = formatContextAsMarkdown(context);
    
    assert(markdown.includes('## Internal Relationships'));
    assert(markdown.includes('bn-test1'));
    assert(markdown.includes('bn-test2'));
    assert(markdown.includes('depends_on'));
});

test('formatContextAsMarkdown includes external connections', () => {
    setEntities('tasks', [
        { id: 'bn-test1', type: 'task', title: 'Task 1' },
        { id: 'bn-test2', type: 'task', title: 'Task 2', status: 'done' }
    ]);
    
    setEdges([
        { source: 'bn-test1', target: 'bn-test2', type: 'depends_on' }
    ]);
    
    setSelectedNodes(['bn-test1']);
    
    const context = gatherSelectionContext();
    const markdown = formatContextAsMarkdown(context);
    
    assert(markdown.includes('## External Connections'));
    assert(markdown.includes('bn-test2'));
    assert(markdown.includes('[done]'));
});

test('formatContextAsMarkdown includes recent activity', () => {
    setEntities('tasks', [
        { id: 'bn-test1', type: 'task', title: 'Task 1' }
    ]);
    
    set('log', [
        {
            timestamp: '2026-01-27T10:00:00Z',
            entity_id: 'bn-test1',
            action: 'update',
            field: 'status',
            actor: 'agent-123',
            message: 'Status changed'
        }
    ]);
    
    setSelectedNodes(['bn-test1']);
    
    const context = gatherSelectionContext();
    const markdown = formatContextAsMarkdown(context);
    
    assert(markdown.includes('## Recent Activity'));
    assert(markdown.includes('bn-test1'));
    assert(markdown.includes('update'));
    assert(markdown.includes('agent-123'));
    assert(markdown.includes('Status changed'));
});

test('formatContextAsJSON produces valid JSON', () => {
    setEntities('tasks', [
        { id: 'bn-test1', type: 'task', title: 'Task 1' }
    ]);
    
    setSelectedNodes(['bn-test1']);
    
    const context = gatherSelectionContext();
    const json = formatContextAsJSON(context);
    
    // Should be parseable
    const parsed = JSON.parse(json);
    assertEquals(parsed.selectionCount, 1);
    assert(parsed.entities[0].id === 'bn-test1');
});

test('Works with mixed entity types', () => {
    setEntities('tasks', [
        { id: 'bn-task1', type: 'task', title: 'Task 1' }
    ]);
    setEntities('bugs', [
        { id: 'bn-bug1', type: 'bug', title: 'Bug 1' }
    ]);
    setEntities('ideas', [
        { id: 'bn-idea1', type: 'idea', title: 'Idea 1' }
    ]);
    setEntities('tests', [
        { id: 'bnt-test1', type: 'test', name: 'Test 1' }
    ]);
    
    setSelectedNodes(['bn-task1', 'bn-bug1', 'bn-idea1', 'bnt-test1']);
    
    const context = gatherSelectionContext();
    
    assertEquals(context.selectionCount, 4);
    assertEquals(context.entities.length, 4);
    assertEquals(context.metadata.entityTypes.task, 1);
    assertEquals(context.metadata.entityTypes.bug, 1);
    assertEquals(context.metadata.entityTypes.idea, 1);
    assertEquals(context.metadata.entityTypes.test, 1);
});

// Summary
console.log(`\n${testsPassed} passed, ${testsFailed} failed`);
process.exit(testsFailed > 0 ? 1 : 0);
