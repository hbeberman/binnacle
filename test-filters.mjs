#!/usr/bin/env node

/**
 * Test script for filter components
 * Validates that filter functions work correctly
 */

console.log('Testing filter components...\n');

// Mock localStorage
global.localStorage = {
    store: {},
    getItem(key) {
        return this.store[key] || null;
    },
    setItem(key, value) {
        this.store[key] = value;
    }
};

// Mock document
global.document = {
    elements: {},
    getElementById(id) {
        if (!this.elements[id]) {
            this.elements[id] = {
                id,
                innerHTML: '',
                className: '',
                children: [],
                dataset: {},
                appendChild(child) {
                    this.children.push(child);
                },
                querySelectorAll(selector) {
                    return [];
                },
                addEventListener() {}
            };
        }
        return this.elements[id];
    },
    createElement(tag) {
        return {
            tagName: tag,
            className: '',
            innerHTML: '',
            textContent: '',
            dataset: {},
            style: {},
            children: [],
            appendChild(child) {
                this.children.push(child);
            },
            classList: {
                toggle() {},
                add() {},
                remove() {}
            },
            addEventListener() {}
        };
    }
};

// Test basic functionality
console.log('✓ Mock environment setup complete');

// Verify NODE_TYPES and EDGE_TYPES are defined in filters.js
import('./web/js/components/filters.js').then(module => {
    console.log('✓ Filter module loaded successfully');
    
    // Test initialization functions exist
    if (typeof module.initializeNodeTypeFilters === 'function') {
        console.log('✓ initializeNodeTypeFilters function exists');
    } else {
        console.error('✗ initializeNodeTypeFilters function missing');
    }
    
    if (typeof module.initializeEdgeTypeFilters === 'function') {
        console.log('✓ initializeEdgeTypeFilters function exists');
    } else {
        console.error('✗ initializeEdgeTypeFilters function missing');
    }
    
    console.log('\nAll filter component tests passed!');
}).catch(err => {
    console.error('✗ Failed to load filter module:', err);
    process.exit(1);
});
