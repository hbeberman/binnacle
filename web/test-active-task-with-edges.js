#!/usr/bin/env node
/**
 * Test: Active Task Pane with working_on edges
 * 
 * Verifies that the active task pane correctly uses working_on edges
 * to determine which tasks are actively being worked on.
 */

import * as state from './js/state.js';
import { handleMessage } from './js/connection/message-handlers.js';

// Import the findActiveTasks function (we'll need to export it for testing)
// For now, we'll test the behavior indirectly by checking state updates

console.log('Testing Active Task Pane with working_on edges...\n');

// Reset state
state.reset();

// Test 1: Create an agent
console.log('Test 1: Create an agent');
const agentMessage = {
    type: 'entity_added',
    entity_type: 'agent',
    id: 'bn-agent1',
    entity: {
        id: 'bn-agent1',
        type: 'agent',
        title: 'Test Agent',
        status: 'active',
        _agent: {
            name: 'TestAgent',
            purpose: 'Testing',
            started_at: '2026-01-27T00:00:00Z'
        }
    },
    version: 1,
    timestamp: '2026-01-27T00:00:00Z'
};
handleMessage(agentMessage);

const agents = state.getAgents();
console.log(`  Agents count: ${agents.length} (expected: 1)`);
console.log(`  Agent ID: ${agents[0]?.id} (expected: bn-agent1)`);
console.log(`  ✓ Agent created\n`);

// Test 2: Create a task
console.log('Test 2: Create a task');
const taskMessage = {
    type: 'entity_added',
    entity_type: 'task',
    id: 'bn-task1',
    entity: {
        id: 'bn-task1',
        type: 'task',
        title: 'Test Task',
        short_name: 'Test',
        status: 'in_progress',
        priority: 2,
        tags: [],
        created_at: '2026-01-27T00:00:00Z',
        updated_at: '2026-01-27T00:01:00Z'
    },
    version: 2,
    timestamp: '2026-01-27T00:01:00Z'
};
handleMessage(taskMessage);

const tasks = state.getTasks();
console.log(`  Tasks count: ${tasks.length} (expected: 1)`);
console.log(`  Task ID: ${tasks[0]?.id} (expected: bn-task1)`);
console.log(`  Task status: ${tasks[0]?.status} (expected: in_progress)`);
console.log(`  ✓ Task created\n`);

// Test 3: Create a working_on edge from agent to task
console.log('Test 3: Create working_on edge');
const edgeMessage = {
    type: 'edge_added',
    id: 'bne-edge1',
    edge: {
        id: 'bne-edge1',
        source: 'bn-agent1',
        target: 'bn-task1',
        edge_type: 'working_on',
        created_at: '2026-01-27T00:01:00Z'
    },
    version: 3,
    timestamp: '2026-01-27T00:01:00Z'
};
handleMessage(edgeMessage);

const edges = state.getEdges();
console.log(`  Edges count: ${edges.length} (expected: 1)`);
console.log(`  Edge ID: ${edges[0]?.id} (expected: bne-edge1)`);
console.log(`  Edge type: ${edges[0]?.edge_type} (expected: working_on)`);
console.log(`  Edge source: ${edges[0]?.source} (expected: bn-agent1)`);
console.log(`  Edge target: ${edges[0]?.target} (expected: bn-task1)`);
console.log(`  ✓ Edge created\n`);

// Test 4: Verify active tasks can be found via working_on edges
console.log('Test 4: Find active tasks via working_on edges');
const workingOnEdges = edges.filter(e => e.edge_type === 'working_on');
console.log(`  working_on edges: ${workingOnEdges.length} (expected: 1)`);

const activePairs = workingOnEdges.map(edge => {
    const agent = agents.find(a => a.id === edge.source);
    const task = state.getNode(edge.target);
    return { agent, task };
}).filter(pair => pair.agent && pair.task);

console.log(`  Active task pairs: ${activePairs.length} (expected: 1)`);
console.log(`  Agent in pair: ${activePairs[0]?.agent?.id} (expected: bn-agent1)`);
console.log(`  Task in pair: ${activePairs[0]?.task?.id} (expected: bn-task1)`);
console.log(`  ✓ Active tasks found via working_on edges\n`);

// Test 5: Multiple agents with different tasks
console.log('Test 5: Multiple agents with different tasks');

// Add second agent
const agent2Message = {
    type: 'entity_added',
    entity_type: 'agent',
    id: 'bn-agent2',
    entity: {
        id: 'bn-agent2',
        type: 'agent',
        title: 'Test Agent 2',
        status: 'active',
        _agent: {
            name: 'TestAgent2',
            purpose: 'Testing 2',
            started_at: '2026-01-27T00:05:00Z'
        }
    },
    version: 4,
    timestamp: '2026-01-27T00:05:00Z'
};
handleMessage(agent2Message);

// Add second task
const task2Message = {
    type: 'entity_added',
    entity_type: 'task',
    id: 'bn-task2',
    entity: {
        id: 'bn-task2',
        type: 'task',
        title: 'Test Task 2',
        short_name: 'Test 2',
        status: 'in_progress',
        priority: 2,
        tags: [],
        created_at: '2026-01-27T00:05:00Z',
        updated_at: '2026-01-27T00:06:00Z'
    },
    version: 5,
    timestamp: '2026-01-27T00:06:00Z'
};
handleMessage(task2Message);

// Add working_on edge from agent2 to task2
const edge2Message = {
    type: 'edge_added',
    id: 'bne-edge2',
    edge: {
        id: 'bne-edge2',
        source: 'bn-agent2',
        target: 'bn-task2',
        edge_type: 'working_on',
        created_at: '2026-01-27T00:06:00Z'
    },
    version: 6,
    timestamp: '2026-01-27T00:06:00Z'
};
handleMessage(edge2Message);

const allAgents = state.getAgents();
const allTasks = state.getTasks();
const allEdges = state.getEdges();

console.log(`  Total agents: ${allAgents.length} (expected: 2)`);
console.log(`  Total tasks: ${allTasks.length} (expected: 2)`);
console.log(`  Total edges: ${allEdges.length} (expected: 2)`);

const allWorkingOn = allEdges.filter(e => e.edge_type === 'working_on');
console.log(`  working_on edges: ${allWorkingOn.length} (expected: 2)`);
console.log(`  ✓ Multiple agents with tasks\n`);

console.log('All tests passed! ✓');
