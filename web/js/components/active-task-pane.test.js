/**
 * Test for Active Task Pane Component
 * 
 * Validates the core functionality without a browser.
 */

// Mock state module
const mockState = {
    tasks: [],
    bugs: [],
    mode: 'websocket',
    listeners: new Map(),
    
    subscribe(path, callback) {
        if (!this.listeners.has(path)) {
            this.listeners.set(path, []);
        }
        this.listeners.get(path).push(callback);
    },
    
    getTasks() {
        return this.tasks;
    },
    
    getBugs() {
        return this.bugs;
    },
    
    getMode() {
        return this.mode;
    }
};

// Test formatDuration function
function formatDuration(ms) {
    const seconds = Math.floor(ms / 1000);
    const minutes = Math.floor(seconds / 60);
    const hours = Math.floor(minutes / 60);
    const days = Math.floor(hours / 24);
    
    if (days > 0) {
        const remainingHours = hours % 24;
        return remainingHours > 0 ? `${days}d ${remainingHours}h` : `${days}d`;
    }
    if (hours > 0) {
        const remainingMinutes = minutes % 60;
        return remainingMinutes > 0 ? `${hours}h ${remainingMinutes}m` : `${hours}h`;
    }
    if (minutes > 0) {
        return `${minutes}m`;
    }
    return `${seconds}s`;
}

// Test calculateElapsed function
function calculateElapsed(updatedAt) {
    if (!updatedAt) return 0;
    const startTime = new Date(updatedAt);
    const now = new Date();
    return now - startTime;
}

console.log('Testing Active Task Pane Component Logic\n');

// Test 1: Duration formatting
console.log('Test 1: Duration formatting');
const tests = [
    { ms: 30000, expected: '30s' },
    { ms: 90000, expected: '1m' },
    { ms: 150000, expected: '2m' },
    { ms: 3600000, expected: '1h' },
    { ms: 3900000, expected: '1h 5m' },
    { ms: 7200000, expected: '2h' },
    { ms: 86400000, expected: '1d' },
    { ms: 90000000, expected: '1d 1h' }
];

let passed = 0;
let failed = 0;

tests.forEach(({ ms, expected }) => {
    const result = formatDuration(ms);
    const match = result === expected;
    if (match) {
        passed++;
        console.log(`  ✓ ${ms}ms → "${result}"`);
    } else {
        failed++;
        console.log(`  ✗ ${ms}ms → "${result}" (expected "${expected}")`);
    }
});

// Test 2: Elapsed time calculation
console.log('\nTest 2: Elapsed time calculation');
const fiveMinutesAgo = new Date(Date.now() - 5 * 60 * 1000).toISOString();
const elapsed = calculateElapsed(fiveMinutesAgo);
const elapsedMinutes = Math.floor(elapsed / 60000);

if (elapsedMinutes === 5) {
    passed++;
    console.log(`  ✓ Elapsed time: ${elapsedMinutes} minutes`);
} else {
    failed++;
    console.log(`  ✗ Elapsed time: ${elapsedMinutes} minutes (expected 5)`);
}

// Test 3: Finding active task
console.log('\nTest 3: Finding active task');
mockState.tasks = [
    { id: 'bn-1', status: 'pending' },
    { id: 'bn-2', status: 'in_progress' },
    { id: 'bn-3', status: 'done' }
];

const activeTask = mockState.tasks.find(t => t.status === 'in_progress');
if (activeTask && activeTask.id === 'bn-2') {
    passed++;
    console.log(`  ✓ Found active task: ${activeTask.id}`);
} else {
    failed++;
    console.log(`  ✗ Active task not found correctly`);
}

// Test 4: Finding active bug
console.log('\nTest 4: Finding active bug');
mockState.tasks = [
    { id: 'bn-1', status: 'pending' }
];
mockState.bugs = [
    { id: 'bn-bug1', status: 'pending' },
    { id: 'bn-bug2', status: 'in_progress' },
    { id: 'bn-bug3', status: 'done' }
];

const activeBug = mockState.bugs.find(b => b.status === 'in_progress');
if (activeBug && activeBug.id === 'bn-bug2') {
    passed++;
    console.log(`  ✓ Found active bug: ${activeBug.id}`);
} else {
    failed++;
    console.log(`  ✗ Active bug not found correctly`);
}

// =============================================================================
// Stale Task Filtering Tests
// =============================================================================

// Mock implementation of recently-removed tracking (mirrors active-task-pane.js)
const REMOVAL_GRACE_PERIOD_MS = 5000;
const recentlyRemovedTargets = new Map();

function markRecentlyUnlinked(taskId) {
    recentlyRemovedTargets.set(taskId, Date.now());
}

function isRecentlyUnlinked(taskId) {
    const removedAt = recentlyRemovedTargets.get(taskId);
    if (!removedAt) return false;
    return (Date.now() - removedAt) < REMOVAL_GRACE_PERIOD_MS;
}

function clearRecentlyUnlinked() {
    recentlyRemovedTargets.clear();
}

// Mock findActiveTasks that incorporates the filtering logic
function findActiveTasksWithFiltering(tasks, bugs, edges = [], agents = []) {
    // Find all working_on edges (simplified mock)
    const workingOnEdges = edges.filter(e => e.edge_type === 'working_on');
    
    // Map each edge to {agent, task, startTime} tuple
    const activePairs = workingOnEdges.map(edge => {
        const agent = agents.find(a => a.id === edge.source);
        const task = [...tasks, ...bugs].find(t => t.id === edge.target);
        
        if (agent && task && (task.status === 'pending' || task.status === 'in_progress')) {
            return { agent, task, startTime: edge.created_at };
        }
        return null;
    }).filter(pair => pair !== null);
    
    // If no working_on edges found, fall back to in_progress tasks
    if (activePairs.length === 0) {
        // Filter out recently-unlinked tasks from the fallback path
        const inProgressTasks = [
            ...tasks.filter(t => t.status === 'in_progress'),
            ...bugs.filter(b => b.status === 'in_progress')
        ].filter(task => !isRecentlyUnlinked(task.id));
        
        return inProgressTasks.map(task => ({ 
            agent: null, 
            task,
            startTime: task.updated_at 
        }));
    }
    
    return activePairs;
}

// Test 5: Recently-unlinked tasks excluded from fallback
console.log('\nTest 5: Recently-unlinked tasks excluded from fallback');
clearRecentlyUnlinked();
mockState.tasks = [
    { id: 'bn-active', status: 'in_progress', updated_at: new Date().toISOString() }
];
mockState.bugs = [];

// Before marking as unlinked, task should appear
let result = findActiveTasksWithFiltering(mockState.tasks, mockState.bugs);
if (result.length === 1 && result[0].task.id === 'bn-active') {
    passed++;
    console.log(`  ✓ Task appears before being marked as unlinked`);
} else {
    failed++;
    console.log(`  ✗ Task should appear before being marked (got ${result.length} tasks)`);
}

// Mark as recently unlinked
markRecentlyUnlinked('bn-active');
result = findActiveTasksWithFiltering(mockState.tasks, mockState.bugs);
if (result.length === 0) {
    passed++;
    console.log(`  ✓ Recently-unlinked task is excluded from fallback`);
} else {
    failed++;
    console.log(`  ✗ Recently-unlinked task should be excluded (got ${result.length} tasks)`);
}

// Test 6: Tasks reappear after grace period expires
console.log('\nTest 6: Tasks reappear after grace period expires');
clearRecentlyUnlinked();
mockState.tasks = [
    { id: 'bn-reappear', status: 'in_progress', updated_at: new Date().toISOString() }
];

// Mark as unlinked with a timestamp in the past (beyond grace period)
recentlyRemovedTargets.set('bn-reappear', Date.now() - REMOVAL_GRACE_PERIOD_MS - 100);

result = findActiveTasksWithFiltering(mockState.tasks, mockState.bugs);
if (result.length === 1 && result[0].task.id === 'bn-reappear') {
    passed++;
    console.log(`  ✓ Task reappears after grace period expires`);
} else {
    failed++;
    console.log(`  ✗ Task should reappear after grace period (got ${result.length} tasks)`);
}

// Test 7: Legitimate in_progress tasks still shown (not recently unlinked)
console.log('\nTest 7: Legitimate in_progress tasks still shown');
clearRecentlyUnlinked();
mockState.tasks = [
    { id: 'bn-legit1', status: 'in_progress', updated_at: new Date().toISOString() },
    { id: 'bn-legit2', status: 'in_progress', updated_at: new Date().toISOString() },
    { id: 'bn-pending', status: 'pending', updated_at: new Date().toISOString() },
    { id: 'bn-done', status: 'done', updated_at: new Date().toISOString() }
];
mockState.bugs = [];

result = findActiveTasksWithFiltering(mockState.tasks, mockState.bugs);
if (result.length === 2) {
    const ids = result.map(r => r.task.id).sort();
    if (ids[0] === 'bn-legit1' && ids[1] === 'bn-legit2') {
        passed++;
        console.log(`  ✓ Both in_progress tasks shown (${ids.join(', ')})`);
    } else {
        failed++;
        console.log(`  ✗ Wrong tasks returned: ${ids.join(', ')}`);
    }
} else {
    failed++;
    console.log(`  ✗ Expected 2 in_progress tasks, got ${result.length}`);
}

// Test 8: Edge cases - multiple tasks, some unlinked
console.log('\nTest 8: Edge cases - multiple tasks, some recently unlinked');
clearRecentlyUnlinked();
mockState.tasks = [
    { id: 'bn-show1', status: 'in_progress', updated_at: new Date().toISOString() },
    { id: 'bn-hide', status: 'in_progress', updated_at: new Date().toISOString() },
    { id: 'bn-show2', status: 'in_progress', updated_at: new Date().toISOString() }
];
mockState.bugs = [
    { id: 'bn-bug-hide', status: 'in_progress', updated_at: new Date().toISOString() },
    { id: 'bn-bug-show', status: 'in_progress', updated_at: new Date().toISOString() }
];

// Mark some as recently unlinked
markRecentlyUnlinked('bn-hide');
markRecentlyUnlinked('bn-bug-hide');

result = findActiveTasksWithFiltering(mockState.tasks, mockState.bugs);
const resultIds = result.map(r => r.task.id).sort();
const expectedIds = ['bn-bug-show', 'bn-show1', 'bn-show2'];

if (resultIds.length === 3 && 
    resultIds[0] === expectedIds[0] && 
    resultIds[1] === expectedIds[1] &&
    resultIds[2] === expectedIds[2]) {
    passed++;
    console.log(`  ✓ Correct tasks shown: ${resultIds.join(', ')}`);
} else {
    failed++;
    console.log(`  ✗ Expected [${expectedIds.join(', ')}], got [${resultIds.join(', ')}]`);
}

// Test 9: Working_on edges bypass recently-unlinked filter
console.log('\nTest 9: Working_on edges bypass recently-unlinked filter');
clearRecentlyUnlinked();
mockState.tasks = [
    { id: 'bn-edge-task', status: 'in_progress', updated_at: new Date().toISOString() }
];
mockState.bugs = [];
const mockEdges = [
    { edge_type: 'working_on', source: 'agent-1', target: 'bn-edge-task', created_at: new Date().toISOString() }
];
const mockAgents = [
    { id: 'agent-1', title: 'Test Agent' }
];

// Mark task as recently unlinked (but there's an active edge, so it should still show)
markRecentlyUnlinked('bn-edge-task');

result = findActiveTasksWithFiltering(mockState.tasks, mockState.bugs, mockEdges, mockAgents);
if (result.length === 1 && result[0].task.id === 'bn-edge-task' && result[0].agent.id === 'agent-1') {
    passed++;
    console.log(`  ✓ Task with working_on edge shown despite being "recently unlinked"`);
} else {
    failed++;
    console.log(`  ✗ Task with working_on edge should be shown (got ${result.length} tasks)`);
}

// Test 10: isRecentlyUnlinked returns false for unknown tasks
console.log('\nTest 10: isRecentlyUnlinked returns false for unknown tasks');
clearRecentlyUnlinked();
if (!isRecentlyUnlinked('bn-unknown')) {
    passed++;
    console.log(`  ✓ Unknown task not marked as recently unlinked`);
} else {
    failed++;
    console.log(`  ✗ Unknown task should not be marked as recently unlinked`);
}

// Test 11: Re-marking a task updates its timestamp
console.log('\nTest 11: Re-marking a task updates its timestamp');
clearRecentlyUnlinked();
// Set an old timestamp
recentlyRemovedTargets.set('bn-remark', Date.now() - REMOVAL_GRACE_PERIOD_MS - 100);

// Task should reappear (grace period expired)
if (!isRecentlyUnlinked('bn-remark')) {
    passed++;
    console.log(`  ✓ Task with expired grace period not marked as recently unlinked`);
} else {
    failed++;
    console.log(`  ✗ Task with expired grace period should not be filtered`);
}

// Re-mark with current timestamp
markRecentlyUnlinked('bn-remark');

// Now should be filtered again
if (isRecentlyUnlinked('bn-remark')) {
    passed++;
    console.log(`  ✓ Re-marked task is now filtered again`);
} else {
    failed++;
    console.log(`  ✗ Re-marked task should be filtered`);
}

// Test 12: Empty state - no tasks, no bugs
console.log('\nTest 12: Empty state - no tasks, no bugs');
clearRecentlyUnlinked();
mockState.tasks = [];
mockState.bugs = [];

result = findActiveTasksWithFiltering(mockState.tasks, mockState.bugs);
if (result.length === 0) {
    passed++;
    console.log(`  ✓ No tasks returned for empty state`);
} else {
    failed++;
    console.log(`  ✗ Expected 0 tasks for empty state, got ${result.length}`);
}

// Summary
console.log('\n' + '='.repeat(50));
console.log(`Tests passed: ${passed}`);
console.log(`Tests failed: ${failed}`);
console.log('='.repeat(50));

process.exit(failed > 0 ? 1 : 0);
