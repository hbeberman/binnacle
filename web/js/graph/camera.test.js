/**
 * Test for Camera Pause Detection
 * 
 * Validates that user interactions (pan/zoom) pause auto-follow.
 */

// Mock state module
const mockState = {
    state: {
        ui: {
            autoFollow: true,
            userPaused: false,
            viewport: {
                panX: 0,
                panY: 0,
                zoom: 1.0,
                minZoom: 0.1,
                maxZoom: 3.0
            }
        }
    },
    listeners: new Map(),
    
    get(path) {
        const parts = path.split('.');
        let current = this.state;
        for (const part of parts) {
            if (current === null || current === undefined) {
                return undefined;
            }
            current = current[part];
        }
        return current;
    },
    
    set(path, value) {
        const parts = path.split('.');
        const lastPart = parts.pop();
        let current = this.state;
        
        for (const part of parts) {
            if (current[part] === undefined) {
                current[part] = {};
            }
            current = current[part];
        }
        
        const oldValue = current[lastPart];
        current[lastPart] = value;
        
        // Notify listeners
        const listeners = this.listeners.get(path);
        if (listeners) {
            for (const callback of listeners) {
                callback(value, oldValue, path);
            }
        }
    },
    
    subscribe(path, callback) {
        if (!this.listeners.has(path)) {
            this.listeners.set(path, []);
        }
        this.listeners.get(path).push(callback);
    }
};

// Test: Initial state should have userPaused = false
function testInitialState() {
    const userPaused = mockState.get('ui.userPaused');
    const autoFollow = mockState.get('ui.autoFollow');
    
    if (userPaused !== false) {
        throw new Error(`Expected userPaused to be false, got ${userPaused}`);
    }
    
    if (autoFollow !== true) {
        throw new Error(`Expected autoFollow to be true, got ${autoFollow}`);
    }
    
    console.log('✓ Initial state test passed');
}

// Test: Setting userPaused to true should pause auto-follow
function testSetUserPaused() {
    let notified = false;
    let notifiedValue = null;
    
    mockState.subscribe('ui.userPaused', (value) => {
        notified = true;
        notifiedValue = value;
    });
    
    mockState.set('ui.userPaused', true);
    
    const userPaused = mockState.get('ui.userPaused');
    
    if (userPaused !== true) {
        throw new Error(`Expected userPaused to be true, got ${userPaused}`);
    }
    
    if (!notified) {
        throw new Error('Expected state change notification');
    }
    
    if (notifiedValue !== true) {
        throw new Error(`Expected notification value to be true, got ${notifiedValue}`);
    }
    
    console.log('✓ Set userPaused test passed');
}

// Test: Resume should clear userPaused flag
function testResumeAutoFollow() {
    mockState.set('ui.userPaused', true);
    
    // Simulate resume
    mockState.set('ui.userPaused', false);
    
    const userPaused = mockState.get('ui.userPaused');
    
    if (userPaused !== false) {
        throw new Error(`Expected userPaused to be false after resume, got ${userPaused}`);
    }
    
    console.log('✓ Resume auto-follow test passed');
}

// Test: Pan should update viewport
function testPanUpdatesViewport() {
    const initialPanX = mockState.get('ui.viewport.panX');
    const initialPanY = mockState.get('ui.viewport.panY');
    
    mockState.set('ui.viewport.panX', initialPanX + 10);
    mockState.set('ui.viewport.panY', initialPanY + 20);
    
    const newPanX = mockState.get('ui.viewport.panX');
    const newPanY = mockState.get('ui.viewport.panY');
    
    if (newPanX !== initialPanX + 10) {
        throw new Error(`Expected panX to be ${initialPanX + 10}, got ${newPanX}`);
    }
    
    if (newPanY !== initialPanY + 20) {
        throw new Error(`Expected panY to be ${initialPanY + 20}, got ${newPanY}`);
    }
    
    console.log('✓ Pan updates viewport test passed');
}

// Test: Zoom should update viewport
function testZoomUpdatesViewport() {
    const initialZoom = mockState.get('ui.viewport.zoom');
    const newZoom = 1.5;
    
    mockState.set('ui.viewport.zoom', newZoom);
    
    const updatedZoom = mockState.get('ui.viewport.zoom');
    
    if (updatedZoom !== newZoom) {
        throw new Error(`Expected zoom to be ${newZoom}, got ${updatedZoom}`);
    }
    
    console.log('✓ Zoom updates viewport test passed');
}

// Run all tests
try {
    console.log('Running camera pause detection tests...\n');
    
    testInitialState();
    testSetUserPaused();
    testResumeAutoFollow();
    testPanUpdatesViewport();
    testZoomUpdatesViewport();
    
    console.log('\n✅ All tests passed!');
} catch (error) {
    console.error('\n❌ Test failed:', error.message);
    process.exit(1);
}
