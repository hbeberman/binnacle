/**
 * Tests for Unified Connection Architecture
 * 
 * Run with: node unified-connection.test.js
 */

import {
    Connection,
    LiveConnection,
    ArchiveConnection,
    createConnection
} from './unified-connection.js';

// Mock state module for testing
const mockState = {
    mode: null,
    connectionStatus: null,
    readonly: false,
    setMode: function(mode, options) {
        this.mode = mode;
    },
    setConnectionStatus: function(status) {
        this.connectionStatus = status;
    },
    set: function(key, value) {
        if (key === 'readonly') {
            this.readonly = value;
        }
    }
};

// Test counter
let passed = 0;
let failed = 0;

function test(name, fn) {
    try {
        fn();
        console.log(`✓ ${name}`);
        passed++;
    } catch (error) {
        console.error(`✗ ${name}`);
        console.error(`  ${error.message}`);
        failed++;
    }
}

function assert(condition, message) {
    if (!condition) {
        throw new Error(message || 'Assertion failed');
    }
}

function assertEquals(actual, expected, message) {
    if (actual !== expected) {
        throw new Error(message || `Expected ${expected}, got ${actual}`);
    }
}

// Run tests
console.log('Running Unified Connection Tests...\n');

// Test 1: Connection base class has required interface
test('Connection base class defines abstract interface', () => {
    const conn = new Connection();
    assert(typeof conn.connect === 'function', 'connect method exists');
    assert(typeof conn.disconnect === 'function', 'disconnect method exists');
    assert(typeof conn.getStatus === 'function', 'getStatus method exists');
    assert(typeof conn.isConnected === 'function', 'isConnected method exists');
    assert(typeof conn.isReadonly === 'function', 'isReadonly method exists');
    assert(typeof conn.send === 'function', 'send method exists');
    assert(typeof conn.on === 'function', 'on method exists');
    assert(typeof conn.off === 'function', 'off method exists');
});

// Test 2: LiveConnection is not readonly
test('LiveConnection isReadonly returns false', () => {
    const conn = new LiveConnection({ wsUrl: 'ws://localhost:55823/ws' });
    assertEquals(conn.isReadonly(), false, 'LiveConnection should be writable');
});

// Test 3: ArchiveConnection is readonly
test('ArchiveConnection isReadonly returns true', () => {
    const conn = new ArchiveConnection({ archiveUrl: 'test.bng' });
    assertEquals(conn.isReadonly(), true, 'ArchiveConnection should be readonly');
});

// Test 4: Event listeners work
test('Event listeners can be added and removed', () => {
    const conn = new Connection();
    let callCount = 0;
    const listener = () => callCount++;
    
    conn.on('test', listener);
    conn._emit('test');
    assertEquals(callCount, 1, 'Listener should be called once');
    
    conn._emit('test');
    assertEquals(callCount, 2, 'Listener should be called twice');
    
    conn.off('test', listener);
    conn._emit('test');
    assertEquals(callCount, 2, 'Listener should not be called after removal');
});

// Test 5: createConnection with live mode
test('createConnection creates LiveConnection for live mode', () => {
    const conn = createConnection({
        mode: 'live',
        wsUrl: 'ws://localhost:55823/ws'
    });
    assert(conn instanceof LiveConnection, 'Should create LiveConnection');
    assertEquals(conn.isReadonly(), false, 'Should be writable');
});

// Test 6: createConnection with archive mode
test('createConnection creates ArchiveConnection for archive mode', () => {
    const conn = createConnection({
        mode: 'archive',
        archiveUrl: 'test.bng'
    });
    assert(conn instanceof ArchiveConnection, 'Should create ArchiveConnection');
    assertEquals(conn.isReadonly(), true, 'Should be readonly');
});

// Test 7: createConnection auto-detects mode from wsUrl
test('createConnection auto-detects live mode from wsUrl', () => {
    const conn = createConnection({
        mode: 'auto',
        wsUrl: 'ws://localhost:55823/ws'
    });
    assert(conn instanceof LiveConnection, 'Should auto-detect LiveConnection');
});

// Test 8: createConnection auto-detects mode from archiveUrl
test('createConnection auto-detects archive mode from archiveUrl', () => {
    const conn = createConnection({
        mode: 'auto',
        archiveUrl: 'test.bng'
    });
    assert(conn instanceof ArchiveConnection, 'Should auto-detect ArchiveConnection');
});

// Test 9: createConnection throws error with invalid mode
test('createConnection throws error for invalid mode', () => {
    let thrown = false;
    try {
        createConnection({
            mode: 'invalid'
        });
    } catch (error) {
        thrown = true;
        assert(error.message.includes('Unknown connection mode'), 'Should throw meaningful error');
    }
    assert(thrown, 'Should throw error for invalid mode');
});

// Test 10: createConnection throws error with no config
test('createConnection throws error with insufficient config', () => {
    let thrown = false;
    try {
        createConnection({
            mode: 'auto'
        });
    } catch (error) {
        thrown = true;
        assert(error.message.includes('Cannot auto-detect'), 'Should throw meaningful error');
    }
    assert(thrown, 'Should throw error with no connection params');
});

// Summary
console.log('\n' + '='.repeat(50));
console.log(`Tests passed: ${passed}`);
console.log(`Tests failed: ${failed}`);
console.log('='.repeat(50));

if (failed > 0) {
    process.exit(1);
}
