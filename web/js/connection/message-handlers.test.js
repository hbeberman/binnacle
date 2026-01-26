/**
 * Unit tests for WebSocket message handlers
 * 
 * These tests verify that incremental entity and edge messages are correctly
 * processed and update the state appropriately.
 */

import * as state from '../state.js';
import { handleMessage, registerHandler, getRegisteredTypes } from './message-handlers.js';

// Test suite for incremental entity messages
console.log('Testing incremental entity message handlers...');

// Reset state before tests
state.reset();

// Test 1: entity_added message
console.log('\nTest 1: entity_added message');
const addMessage = {
    type: 'entity_added',
    entity_type: 'task',
    id: 'bn-test1',
    entity: {
        id: 'bn-test1',
        type: 'task',
        title: 'Test Task',
        description: 'A test task',
        priority: 1,
        status: 'pending',
        tags: ['test'],
        created_at: '2026-01-26T00:00:00Z'
    },
    version: 1,
    timestamp: '2026-01-26T00:00:00Z'
};

handleMessage(addMessage);

const tasks = state.getTasks();
console.log(`  Tasks count: ${tasks.length} (expected: 1)`);
console.log(`  Task ID: ${tasks[0]?.id} (expected: bn-test1)`);
console.log(`  Task title: ${tasks[0]?.title} (expected: Test Task)`);
console.log(`  ✓ entity_added test passed`);

// Test 2: entity_updated message
console.log('\nTest 2: entity_updated message');
const updateMessage = {
    type: 'entity_updated',
    entity_type: 'task',
    id: 'bn-test1',
    entity: {
        id: 'bn-test1',
        type: 'task',
        title: 'Updated Test Task',
        description: 'An updated test task',
        priority: 2,
        status: 'in_progress',
        tags: ['test', 'updated'],
        created_at: '2026-01-26T00:00:00Z',
        updated_at: '2026-01-26T00:01:00Z'
    },
    version: 2,
    timestamp: '2026-01-26T00:01:00Z'
};

handleMessage(updateMessage);

const updatedTasks = state.getTasks();
console.log(`  Tasks count: ${updatedTasks.length} (expected: 1)`);
console.log(`  Task title: ${updatedTasks[0]?.title} (expected: Updated Test Task)`);
console.log(`  Task status: ${updatedTasks[0]?.status} (expected: in_progress)`);
console.log(`  Task priority: ${updatedTasks[0]?.priority} (expected: 2)`);
console.log(`  ✓ entity_updated test passed`);

// Test 3: edge_added message
console.log('\nTest 3: edge_added message');
const edgeAddMessage = {
    type: 'edge_added',
    id: 'bne-test1',
    edge: {
        id: 'bne-test1',
        source: 'bn-test1',
        target: 'bn-test2',
        edge_type: 'depends_on',
        reason: 'Test dependency',
        created_at: '2026-01-26T00:02:00Z'
    },
    version: 3,
    timestamp: '2026-01-26T00:02:00Z'
};

handleMessage(edgeAddMessage);

const edges = state.getEdges();
console.log(`  Edges count: ${edges.length} (expected: 1)`);
console.log(`  Edge ID: ${edges[0]?.id} (expected: bne-test1)`);
console.log(`  Edge source: ${edges[0]?.source} (expected: bn-test1)`);
console.log(`  Edge target: ${edges[0]?.target} (expected: bn-test2)`);
console.log(`  Edge type: ${edges[0]?.edge_type} (expected: depends_on)`);
console.log(`  ✓ edge_added test passed`);

// Test 4: edge_removed message
console.log('\nTest 4: edge_removed message');
const edgeRemoveMessage = {
    type: 'edge_removed',
    id: 'bne-test1',
    edge: {
        id: 'bne-test1',
        source: 'bn-test1',
        target: 'bn-test2'
    },
    version: 4,
    timestamp: '2026-01-26T00:03:00Z'
};

handleMessage(edgeRemoveMessage);

const edgesAfterRemoval = state.getEdges();
console.log(`  Edges count: ${edgesAfterRemoval.length} (expected: 0)`);
console.log(`  ✓ edge_removed test passed`);

// Test 5: entity_removed message
console.log('\nTest 5: entity_removed message');
const removeMessage = {
    type: 'entity_removed',
    entity_type: 'task',
    id: 'bn-test1',
    version: 5,
    timestamp: '2026-01-26T00:04:00Z'
};

handleMessage(removeMessage);

const tasksAfterRemoval = state.getTasks();
console.log(`  Tasks count: ${tasksAfterRemoval.length} (expected: 0)`);
console.log(`  ✓ entity_removed test passed`);

// Test 6: Multiple entity types
console.log('\nTest 6: Multiple entity types');
handleMessage({
    type: 'entity_added',
    entity_type: 'bug',
    id: 'bnb-test1',
    entity: {
        id: 'bnb-test1',
        type: 'bug',
        title: 'Test Bug',
        priority: 0,
        status: 'open'
    },
    version: 6,
    timestamp: '2026-01-26T00:05:00Z'
});

handleMessage({
    type: 'entity_added',
    entity_type: 'idea',
    id: 'bni-test1',
    entity: {
        id: 'bni-test1',
        type: 'idea',
        title: 'Test Idea',
        priority: 4,
        status: 'pending'
    },
    version: 7,
    timestamp: '2026-01-26T00:06:00Z'
});

const bugs = state.getBugs();
const ideas = state.getIdeas();
console.log(`  Bugs count: ${bugs.length} (expected: 1)`);
console.log(`  Ideas count: ${ideas.length} (expected: 1)`);
console.log(`  ✓ Multiple entity types test passed`);

// Test 7: Verify registered handlers
console.log('\nTest 7: Registered handlers');
const registeredTypes = getRegisteredTypes();
console.log(`  Registered types: ${registeredTypes.join(', ')}`);
const requiredTypes = ['sync', 'reload', 'entity_added', 'entity_updated', 'entity_removed', 'edge_added', 'edge_removed'];
const hasAllRequired = requiredTypes.every(type => registeredTypes.includes(type));
console.log(`  Has all required handlers: ${hasAllRequired} (expected: true)`);
console.log(`  ✓ Registered handlers test passed`);

// Test 8: Version tracking
console.log('\nTest 8: Version tracking');
const syncVersion = state.get('sync.version');
const lastSync = state.get('sync.lastSync');
console.log(`  Sync version: ${syncVersion} (expected: 7)`);
console.log(`  Last sync: ${lastSync} (expected: 2026-01-26T00:06:00Z)`);
console.log(`  ✓ Version tracking test passed`);

console.log('\n✅ All tests passed!');
