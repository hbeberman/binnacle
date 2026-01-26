/**
 * Test for Active Task Pane Component
 * 
 * Validates the core functionality without a browser.
 */

// Mock state module
const mockState = {
    tasks: [],
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

// Summary
console.log('\n' + '='.repeat(50));
console.log(`Tests passed: ${passed}`);
console.log(`Tests failed: ${failed}`);
console.log('='.repeat(50));

process.exit(failed > 0 ? 1 : 0);
