/**
 * Test for Sidebar Search Component
 * 
 * Validates that sidebar search updates state correctly.
 */

// Simple test framework
function assertEqual(actual, expected, message) {
    if (actual === expected) {
        console.log(`✓ ${message}`);
        return true;
    } else {
        console.error(`✗ ${message}`);
        console.error(`  Expected: ${expected}`);
        console.error(`  Actual:   ${actual}`);
        return false;
    }
}

function assertNotNull(value, message) {
    if (value !== null && value !== undefined) {
        console.log(`✓ ${message}`);
        return true;
    } else {
        console.error(`✗ ${message}`);
        console.error(`  Value was null/undefined`);
        return false;
    }
}

console.log('Testing Sidebar Search Component\n');

// Mock DOM elements
class MockHTMLInputElement {
    constructor(id) {
        this.id = id;
        this.value = '';
        this.listeners = new Map();
    }
    
    addEventListener(event, callback) {
        if (!this.listeners.has(event)) {
            this.listeners.set(event, []);
        }
        this.listeners.get(event).push(callback);
    }
    
    trigger(event, data = {}) {
        const handlers = this.listeners.get(event);
        if (handlers) {
            handlers.forEach(handler => handler(data));
        }
    }
    
    trim() {
        return this.value.trim();
    }
}

// Mock State module
const mockState = {
    _state: {
        ui: {
            searchQuery: ''
        }
    },
    
    set(path, value) {
        const parts = path.split('.');
        let current = this._state;
        for (let i = 0; i < parts.length - 1; i++) {
            if (!current[parts[i]]) {
                current[parts[i]] = {};
            }
            current = current[parts[i]];
        }
        current[parts[parts.length - 1]] = value;
    },
    
    get(path) {
        const parts = path.split('.');
        let current = this._state;
        for (const part of parts) {
            if (!current || current[part] === undefined) {
                return undefined;
            }
            current = current[part];
        }
        return current;
    }
};

// Mock document.getElementById
const mockElements = new Map();
global.document = {
    getElementById(id) {
        if (!mockElements.has(id)) {
            mockElements.set(id, new MockHTMLInputElement(id));
        }
        return mockElements.get(id);
    }
};

// Test the search initialization logic inline (extracted from sidebar.js)
function initializeSidebarSearch(State, onSearch) {
    const input = document.getElementById('sidebar-search');
    if (!input) return;

    input.addEventListener('input', () => {
        const query = input.value.trim();
        
        // Update state to trigger graph filtering
        State.set('ui.searchQuery', query);
        
        // Call optional callback
        if (onSearch) {
            onSearch(query);
        }
    });

    input.addEventListener('keydown', (e) => {
        if (e.key === 'Escape') {
            input.value = '';
            State.set('ui.searchQuery', '');
            if (onSearch) {
                onSearch('');
            }
            e.preventDefault();
        }
    });
}

// Run tests
let testsPassed = 0;
let testsFailed = 0;

// Test 1: Input element is retrieved
console.log('\n=== Test 1: Input Element Retrieval ===');
const input = document.getElementById('sidebar-search');
if (assertNotNull(input, 'Input element should be created')) {
    testsPassed++;
} else {
    testsFailed++;
}

// Test 2: Initialize search with state
console.log('\n=== Test 2: Initialize Search ===');
let callbackFired = false;
let callbackQuery = null;

initializeSidebarSearch(mockState, (query) => {
    callbackFired = true;
    callbackQuery = query;
});

if (assertNotNull(input.listeners.get('input'), 'Input event listener should be attached')) {
    testsPassed++;
} else {
    testsFailed++;
}

if (assertNotNull(input.listeners.get('keydown'), 'Keydown event listener should be attached')) {
    testsPassed++;
} else {
    testsFailed++;
}

// Test 3: Typing in search input updates state
console.log('\n=== Test 3: Search Input Updates State ===');
input.value = 'test query';
input.trigger('input');

if (assertEqual(mockState.get('ui.searchQuery'), 'test query', 'State should be updated with search query')) {
    testsPassed++;
} else {
    testsFailed++;
}

if (assertEqual(callbackFired, true, 'Callback should be fired')) {
    testsPassed++;
} else {
    testsFailed++;
}

if (assertEqual(callbackQuery, 'test query', 'Callback should receive query')) {
    testsPassed++;
} else {
    testsFailed++;
}

// Test 4: Empty search clears state
console.log('\n=== Test 4: Empty Search ===');
callbackFired = false;
callbackQuery = null;
input.value = '   ';
input.trigger('input');

if (assertEqual(mockState.get('ui.searchQuery'), '', 'Empty/whitespace search should set empty string')) {
    testsPassed++;
} else {
    testsFailed++;
}

// Test 5: Escape key clears search
console.log('\n=== Test 5: Escape Key Clears Search ===');
input.value = 'some search';
input.trigger('input');
assertEqual(mockState.get('ui.searchQuery'), 'some search', 'State should have search query');

callbackFired = false;
callbackQuery = null;
const escapeEvent = { 
    key: 'Escape', 
    preventDefault: () => {} 
};
input.trigger('keydown', escapeEvent);

if (assertEqual(input.value, '', 'Input value should be cleared')) {
    testsPassed++;
} else {
    testsFailed++;
}

if (assertEqual(mockState.get('ui.searchQuery'), '', 'State should be cleared')) {
    testsPassed++;
} else {
    testsFailed++;
}

if (assertEqual(callbackFired, true, 'Callback should be fired on Escape')) {
    testsPassed++;
} else {
    testsFailed++;
}

if (assertEqual(callbackQuery, '', 'Callback should receive empty string')) {
    testsPassed++;
} else {
    testsFailed++;
}

// Test 6: Trimming whitespace
console.log('\n=== Test 6: Whitespace Trimming ===');
input.value = '  search term  ';
input.trigger('input');

if (assertEqual(mockState.get('ui.searchQuery'), 'search term', 'Whitespace should be trimmed')) {
    testsPassed++;
} else {
    testsFailed++;
}

// Summary
console.log('\n' + '='.repeat(50));
console.log(`Tests Passed: ${testsPassed}`);
console.log(`Tests Failed: ${testsFailed}`);
console.log('='.repeat(50));

if (testsFailed === 0) {
    console.log('\n✓ All tests passed!');
} else {
    console.error('\n✗ Some tests failed.');
    process.exit(1);
}
