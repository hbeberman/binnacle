/**
 * Test for Pan-to-Node Animation
 * 
 * Validates smooth animated camera panning with easing.
 */

// Mock state module
const mockState = {
    state: {
        ui: {
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
    }
};

// Mock requestAnimationFrame
let rafCallbacks = [];
let rafId = 0;
global.requestAnimationFrame = (callback) => {
    const id = ++rafId;
    rafCallbacks.push({ id, callback });
    return id;
};

global.cancelAnimationFrame = (id) => {
    rafCallbacks = rafCallbacks.filter(item => item.id !== id);
};

global.performance = {
    now: () => Date.now()
};

// Mock canvas
const mockCanvas = {
    width: 1000,
    height: 800
};

// Simplified versions of transform functions for testing
function worldToScreen(worldX, worldY, canvas) {
    const viewport = mockState.get('ui.viewport');
    const { panX, panY, zoom } = viewport;
    
    return {
        x: (worldX + panX) * zoom + canvas.width / 2,
        y: (worldY + panY) * zoom + canvas.height / 2
    };
}

// Ease-in-out-cubic timing function
function easeInOutCubic(t) {
    return t < 0.5 
        ? 4 * t * t * t 
        : 1 - Math.pow(-2 * t + 2, 3) / 2;
}

// Calculate adaptive animation duration based on distance
function calculateDuration(distance) {
    if (distance < 300) return 300;
    if (distance < 800) return 500;
    return 800;
}

// Animation state
let panAnimation = null;

// Pan-to-node function (simplified for testing)
function panToNode(targetX, targetY, options = {}) {
    // Cancel any existing animation
    if (panAnimation) {
        cancelAnimationFrame(panAnimation.frameId);
    }
    
    const viewport = mockState.get('ui.viewport');
    const startPanX = viewport.panX;
    const startPanY = viewport.panY;
    const startZoom = viewport.zoom;
    
    const targetPanX = -targetX;
    const targetPanY = -targetY;
    const targetZoom = options.targetZoom !== undefined ? options.targetZoom : startZoom;
    
    // Calculate distance for adaptive duration
    let duration = options.duration;
    if (duration === undefined && options.canvas) {
        const canvas = options.canvas;
        const startScreen = worldToScreen(targetX, targetY, canvas);
        const centerX = canvas.width / 2;
        const centerY = canvas.height / 2;
        const distance = Math.sqrt(
            Math.pow(startScreen.x - centerX, 2) + 
            Math.pow(startScreen.y - centerY, 2)
        );
        duration = calculateDuration(distance);
    } else if (duration === undefined) {
        duration = 500;
    }
    
    const startTime = performance.now();
    
    panAnimation = {
        frameId: null,
        cancelled: false,
        duration,
        startTime
    };
    
    function animate(currentTime) {
        if (panAnimation.cancelled) {
            return;
        }
        
        const elapsed = currentTime - startTime;
        const progress = Math.min(elapsed / duration, 1.0);
        const eased = easeInOutCubic(progress);
        
        const currentPanX = startPanX + (targetPanX - startPanX) * eased;
        const currentPanY = startPanY + (targetPanY - startPanY) * eased;
        const currentZoom = startZoom + (targetZoom - startZoom) * eased;
        
        mockState.set('ui.viewport.panX', currentPanX);
        mockState.set('ui.viewport.panY', currentPanY);
        mockState.set('ui.viewport.zoom', currentZoom);
        
        if (progress < 1.0) {
            panAnimation.frameId = requestAnimationFrame(animate);
        } else {
            panAnimation = null;
            if (options.onComplete) {
                options.onComplete();
            }
        }
    }
    
    panAnimation.frameId = requestAnimationFrame(animate);
    
    return panAnimation;
}

function cancelPanAnimation() {
    if (panAnimation) {
        panAnimation.cancelled = true;
        if (panAnimation.frameId) {
            cancelAnimationFrame(panAnimation.frameId);
        }
        panAnimation = null;
    }
}

// Test: Ease-in-out-cubic produces correct values
function testEasingFunction() {
    const testCases = [
        { t: 0, expected: 0 },
        { t: 0.5, expected: 0.5 },
        { t: 1, expected: 1 }
    ];
    
    for (const { t, expected } of testCases) {
        const result = easeInOutCubic(t);
        const tolerance = 0.001;
        if (Math.abs(result - expected) > tolerance) {
            throw new Error(`Easing function failed for t=${t}: expected ${expected}, got ${result}`);
        }
    }
    
    // Test that easing is smooth (no jumps)
    const t1 = easeInOutCubic(0.25);
    const t2 = easeInOutCubic(0.75);
    if (t1 >= t2) {
        throw new Error('Easing function is not monotonically increasing');
    }
    
    console.log('✓ Easing function test passed');
}

// Test: Duration calculation based on distance
function testDurationCalculation() {
    const testCases = [
        { distance: 100, expected: 300 },
        { distance: 299, expected: 300 },
        { distance: 300, expected: 500 },
        { distance: 500, expected: 500 },
        { distance: 799, expected: 500 },
        { distance: 800, expected: 800 },
        { distance: 1000, expected: 800 }
    ];
    
    for (const { distance, expected } of testCases) {
        const result = calculateDuration(distance);
        if (result !== expected) {
            throw new Error(`Duration calculation failed for distance=${distance}: expected ${expected}ms, got ${result}ms`);
        }
    }
    
    console.log('✓ Duration calculation test passed');
}

// Test: Pan animation initializes correctly
function testPanAnimationInitialization() {
    // Reset state
    mockState.set('ui.viewport.panX', 0);
    mockState.set('ui.viewport.panY', 0);
    mockState.set('ui.viewport.zoom', 1.0);
    rafCallbacks = [];
    
    // Start animation
    const anim = panToNode(100, 200, { duration: 500 });
    
    if (!anim) {
        throw new Error('Pan animation should return animation object');
    }
    
    if (rafCallbacks.length === 0) {
        throw new Error('Animation should schedule a requestAnimationFrame');
    }
    
    // Cancel before executing
    cancelPanAnimation();
    
    console.log('✓ Pan animation initialization test passed');
}

// Test: Pan animation reaches target position
function testPanAnimationReachesTarget() {
    return new Promise((resolve, reject) => {
        // Reset state
        mockState.set('ui.viewport.panX', 0);
        mockState.set('ui.viewport.panY', 0);
        mockState.set('ui.viewport.zoom', 1.0);
        rafCallbacks = [];
        
        const targetX = 100;
        const targetY = 200;
        let completed = false;
        
        // Start animation with onComplete callback
        panToNode(targetX, targetY, {
            duration: 100, // Short duration for testing
            onComplete: () => {
                completed = true;
            }
        });
        
        // Simulate animation frames
        const startTime = performance.now();
        
        function simulateFrames(currentTime) {
            const elapsed = currentTime - startTime;
            
            // Execute all pending RAF callbacks
            const callbacks = [...rafCallbacks];
            rafCallbacks = [];
            
            for (const { callback } of callbacks) {
                callback(currentTime);
            }
            
            if (elapsed < 150 && rafCallbacks.length > 0) {
                // Continue simulation
                setTimeout(() => simulateFrames(startTime + elapsed + 16), 16);
            } else {
                // Animation should be complete
                const finalPanX = mockState.get('ui.viewport.panX');
                const finalPanY = mockState.get('ui.viewport.panY');
                
                const tolerance = 0.1;
                if (Math.abs(finalPanX - (-targetX)) > tolerance) {
                    return reject(new Error(`Final panX should be ${-targetX}, got ${finalPanX}`));
                }
                if (Math.abs(finalPanY - (-targetY)) > tolerance) {
                    return reject(new Error(`Final panY should be ${-targetY}, got ${finalPanY}`));
                }
                
                if (!completed) {
                    return reject(new Error('onComplete callback was not called'));
                }
                
                console.log('✓ Pan animation reaches target test passed');
                resolve();
            }
        }
        
        setTimeout(() => simulateFrames(startTime + 16), 16);
    });
}

// Test: Pan animation can be cancelled
function testPanAnimationCancellation() {
    // Reset state
    mockState.set('ui.viewport.panX', 0);
    mockState.set('ui.viewport.panY', 0);
    rafCallbacks = [];
    
    const initialPanX = mockState.get('ui.viewport.panX');
    
    // Start animation
    panToNode(100, 200, { duration: 1000 });
    
    // Immediately cancel
    cancelPanAnimation();
    
    if (rafCallbacks.length > 0) {
        throw new Error('Cancelled animation should not have pending RAF callbacks');
    }
    
    // Pan should not have moved significantly
    const panX = mockState.get('ui.viewport.panX');
    if (Math.abs(panX - initialPanX) > 10) {
        throw new Error(`Pan should not move significantly after immediate cancel: ${panX} vs ${initialPanX}`);
    }
    
    console.log('✓ Pan animation cancellation test passed');
}

// Test: Pan animation with zoom
function testPanAnimationWithZoom() {
    return new Promise((resolve, reject) => {
        // Reset state
        mockState.set('ui.viewport.panX', 0);
        mockState.set('ui.viewport.panY', 0);
        mockState.set('ui.viewport.zoom', 1.0);
        rafCallbacks = [];
        
        const targetX = 50;
        const targetY = 100;
        const targetZoom = 1.5;
        
        // Start animation with zoom
        panToNode(targetX, targetY, {
            duration: 100,
            targetZoom,
            onComplete: () => {
                const finalZoom = mockState.get('ui.viewport.zoom');
                
                const tolerance = 0.01;
                if (Math.abs(finalZoom - targetZoom) > tolerance) {
                    return reject(new Error(`Final zoom should be ${targetZoom}, got ${finalZoom}`));
                }
                
                console.log('✓ Pan animation with zoom test passed');
                resolve();
            }
        });
        
        // Simulate animation frames
        const startTime = performance.now();
        
        function simulateFrames(currentTime) {
            const elapsed = currentTime - startTime;
            
            const callbacks = [...rafCallbacks];
            rafCallbacks = [];
            
            for (const { callback } of callbacks) {
                callback(currentTime);
            }
            
            if (elapsed < 150 && rafCallbacks.length > 0) {
                setTimeout(() => simulateFrames(startTime + elapsed + 16), 16);
            }
        }
        
        setTimeout(() => simulateFrames(startTime + 16), 16);
    });
}

// Test: Adaptive duration based on distance
function testAdaptiveDuration() {
    // Reset state
    mockState.set('ui.viewport.panX', 0);
    mockState.set('ui.viewport.panY', 0);
    mockState.set('ui.viewport.zoom', 1.0);
    rafCallbacks = [];
    
    // Short distance (should be 300ms)
    const anim1 = panToNode(10, 10, { canvas: mockCanvas });
    if (anim1.duration !== 300) {
        throw new Error(`Short distance should use 300ms, got ${anim1.duration}ms`);
    }
    cancelPanAnimation();
    
    // Medium distance (should be 500ms)
    mockState.set('ui.viewport.panX', 0);
    mockState.set('ui.viewport.panY', 0);
    const anim2 = panToNode(400, 0, { canvas: mockCanvas });
    if (anim2.duration !== 500) {
        throw new Error(`Medium distance should use 500ms, got ${anim2.duration}ms`);
    }
    cancelPanAnimation();
    
    console.log('✓ Adaptive duration test passed');
}

// Run all tests
async function runTests() {
    try {
        console.log('Running pan-to-node animation tests...\n');
        
        testEasingFunction();
        testDurationCalculation();
        testPanAnimationInitialization();
        await testPanAnimationReachesTarget();
        testPanAnimationCancellation();
        await testPanAnimationWithZoom();
        testAdaptiveDuration();
        
        console.log('\n✅ All tests passed!');
    } catch (error) {
        console.error('\n❌ Test failed:', error.message);
        console.error(error.stack);
        process.exit(1);
    }
}

runTests();
