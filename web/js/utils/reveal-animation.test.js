/**
 * Tests for Progressive Reveal Animation
 */

import { animateProgressiveReveal, getRevealOpacity, hasActiveRevealAnimations, clearRevealAnimations } from './reveal-animation.js';

// Simple test helpers
function test(name, fn) {
    return new Promise((resolve) => {
        fn()
            .then(() => {
                console.log(`✓ ${name}`);
                resolve();
            })
            .catch((error) => {
                console.error(`✗ ${name}`);
                console.error(error);
                resolve(); // Don't fail the whole suite
            });
    });
}

function assertEquals(actual, expected, message) {
    if (actual !== expected) {
        throw new Error(message || `Expected: ${expected}, Got: ${actual}`);
    }
}

function assertGreaterThan(actual, expected, message) {
    if (actual <= expected) {
        throw new Error(message || `Expected ${actual} > ${expected}`);
    }
}

function assertLessThan(actual, expected, message) {
    if (actual >= expected) {
        throw new Error(message || `Expected ${actual} < ${expected}`);
    }
}

function assertNull(actual, message) {
    if (actual !== null) {
        throw new Error(message || `Expected null, Got: ${actual}`);
    }
}

function assertNotNull(actual, message) {
    if (actual === null) {
        throw new Error(message || `Expected non-null value`);
    }
}

function sleep(ms) {
    return new Promise(resolve => setTimeout(resolve, ms));
}

// Run tests
async function runTests() {
    console.log('Running Progressive Reveal Animation tests...\n');

    await test('should reveal nodes progressively by depth', async () => {
        clearRevealAnimations();
        const rootId = 'bn-root';
        const descendants = new Set(['bn-root', 'bn-child1', 'bn-child2', 'bn-grandchild']);
        const depthMap = new Map([
            ['bn-root', 0],
            ['bn-child1', 1],
            ['bn-child2', 1],
            ['bn-grandchild', 2]
        ]);
        const existingNodes = new Map();

        const startTime = performance.now();
        await animateProgressiveReveal(rootId, descendants, depthMap, existingNodes);
        const duration = performance.now() - startTime;

        assertGreaterThan(duration, 140, 'Animation should take at least 140ms');
        assertLessThan(duration, 2000, 'Animation should complete in under 2 seconds');
    });

    await test('should not animate already visible nodes', async () => {
        clearRevealAnimations();
        const rootId = 'bn-root';
        const descendants = new Set(['bn-root', 'bn-child1']);
        const depthMap = new Map([
            ['bn-root', 0],
            ['bn-child1', 1]
        ]);
        const existingNodes = new Map([
            ['bn-root', { id: 'bn-root', x: 0, y: 0 }]
        ]);

        await animateProgressiveReveal(rootId, descendants, depthMap, existingNodes);

        assertNull(getRevealOpacity('bn-root'), 'Root should not have animation');
        const childOpacity = getRevealOpacity('bn-child1');
        assertNotNull(childOpacity, 'Child should have animation');
    });

    await test('should return null for nodes without active animation', async () => {
        clearRevealAnimations();
        assertNull(getRevealOpacity('bn-test'));
    });

    await test('should return opacity value during animation', async () => {
        clearRevealAnimations();
        const rootId = 'bn-root';
        const descendants = new Set(['bn-child']);
        const depthMap = new Map([['bn-child', 0]]);
        const existingNodes = new Map();

        const animationPromise = animateProgressiveReveal(rootId, descendants, depthMap, existingNodes);
        await sleep(10);

        const opacity = getRevealOpacity('bn-child');
        assertNotNull(opacity, 'Should have opacity during animation');
        assertGreaterThan(opacity, 0, 'Opacity should be > 0');
        assertLessThan(opacity, 1.1, 'Opacity should be <= 1');

        await animationPromise;
    });

    await test('should return false when no animations are active', async () => {
        clearRevealAnimations();
        assertEquals(hasActiveRevealAnimations(), false);
    });

    await test('should return true during active animations', async () => {
        clearRevealAnimations();
        const rootId = 'bn-root';
        const descendants = new Set(['bn-child']);
        const depthMap = new Map([['bn-child', 0]]);
        const existingNodes = new Map();

        const animationPromise = animateProgressiveReveal(rootId, descendants, depthMap, existingNodes);
        await sleep(10);

        assertEquals(hasActiveRevealAnimations(), true);

        await animationPromise;
    });

    await test('should clear all active animations', async () => {
        clearRevealAnimations();
        const rootId = 'bn-root';
        const descendants = new Set(['bn-child1', 'bn-child2']);
        const depthMap = new Map([
            ['bn-child1', 0],
            ['bn-child2', 1]
        ]);
        const existingNodes = new Map();

        const animationPromise = animateProgressiveReveal(rootId, descendants, depthMap, existingNodes);
        await sleep(10);

        assertEquals(hasActiveRevealAnimations(), true);

        clearRevealAnimations();

        assertEquals(hasActiveRevealAnimations(), false);
        assertNull(getRevealOpacity('bn-child1'));
        assertNull(getRevealOpacity('bn-child2'));

        await animationPromise;
    });

    await test('should respect 75ms delay between depth levels', async () => {
        clearRevealAnimations();
        const rootId = 'bn-root';
        const descendants = new Set(['bn-depth0', 'bn-depth1', 'bn-depth2']);
        const depthMap = new Map([
            ['bn-depth0', 0],
            ['bn-depth1', 1],
            ['bn-depth2', 2]
        ]);
        const existingNodes = new Map();

        const startTime = performance.now();
        await animateProgressiveReveal(rootId, descendants, depthMap, existingNodes);
        const duration = performance.now() - startTime;

        assertGreaterThan(duration, 140, 'Should take at least 140ms');
        assertLessThan(duration, 200, 'Should complete quickly');
    });

    console.log('\n✅ All tests completed');
}

// Auto-run tests if loaded as a module
if (typeof window !== 'undefined') {
    runTests();
}
