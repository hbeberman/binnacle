/**
 * Unit tests for SpatialHash class
 */

import { SpatialHash } from './spatial-hash.js';

console.log('Testing SpatialHash class\n');

// Test 1: Basic grid initialization
console.log('Test 1: Grid initialization');
{
    const hash = new SpatialHash(100);
    if (hash.cellSize === 100) {
        console.log('  ✓ Cell size set correctly');
    } else {
        throw new Error('Cell size not set correctly');
    }
    if (hash.grid instanceof Map) {
        console.log('  ✓ Grid is a Map');
    } else {
        throw new Error('Grid is not a Map');
    }
    console.log('  ✓ Grid initialized correctly\n');
}

// Test 2: Node insertion and retrieval
console.log('Test 2: Node insertion and retrieval');
{
    const hash = new SpatialHash(100);
    const nodes = [
        { id: '1', x: 50, y: 50 },
        { id: '2', x: 150, y: 50 },
        { id: '3', x: 50, y: 150 },
        { id: '4', x: 250, y: 250 }
    ];
    
    hash.rebuild(nodes);
    
    // Node 1 is at (50, 50) -> cell (0, 0)
    // Node 2 is at (150, 50) -> cell (1, 0)
    // Node 3 is at (50, 150) -> cell (0, 1)
    // Node 4 is at (250, 250) -> cell (2, 2)
    
    const nearby1 = hash.getNearby(50, 50, 100);
    if (nearby1.length >= 1) {
        console.log('  ✓ Found nodes near (50, 50)');
    } else {
        throw new Error('Should find at least 1 node near (50, 50)');
    }
    
    const nearby2 = hash.getNearby(250, 250, 50);
    if (nearby2.length >= 1) {
        console.log('  ✓ Found nodes near (250, 250)');
    } else {
        throw new Error('Should find at least 1 node near (250, 250)');
    }
    
    console.log('  ✓ Node insertion and retrieval working\n');
}

// Test 3: Repulsion filtering with cutoff distance
console.log('Test 3: Repulsion filtering with cutoff distance');
{
    const hash = new SpatialHash(150);
    const nodes = [
        { id: '1', x: 0, y: 0 },
        { id: '2', x: 100, y: 0 },   // 100px away
        { id: '3', x: 250, y: 0 },   // 250px away (beyond cutoff)
        { id: '4', x: 0, y: 150 }    // 150px away
    ];
    
    hash.rebuild(nodes);
    
    const nearby = hash.getNearbyForRepulsion(nodes[0], 200);
    
    // Should find node 2 (100px) and node 4 (150px), but not node 3 (250px)
    const foundIds = nearby.map(n => n.id);
    
    if (foundIds.includes('2')) {
        console.log('  ✓ Found node within cutoff (100px)');
    } else {
        throw new Error('Should find node within cutoff');
    }
    
    if (foundIds.includes('4')) {
        console.log('  ✓ Found node at edge of cutoff (150px)');
    } else {
        throw new Error('Should find node at edge of cutoff');
    }
    
    if (!foundIds.includes('3')) {
        console.log('  ✓ Correctly excluded node beyond cutoff (250px)');
    } else {
        throw new Error('Should not find node beyond cutoff');
    }
    
    if (!foundIds.includes('1')) {
        console.log('  ✓ Correctly excluded self');
    } else {
        throw new Error('Should not include self in results');
    }
    
    console.log('  ✓ Repulsion filtering working correctly\n');
}

// Test 4: Performance with many nodes
console.log('Test 4: Performance test with many nodes');
{
    const hash = new SpatialHash(150);
    
    // Create a grid of nodes
    const nodes = [];
    for (let x = 0; x < 1000; x += 100) {
        for (let y = 0; y < 1000; y += 100) {
            nodes.push({ id: `${x},${y}`, x, y });
        }
    }
    
    console.log(`  Testing with ${nodes.length} nodes`);
    
    const start = Date.now();
    hash.rebuild(nodes);
    const rebuildTime = Date.now() - start;
    
    console.log(`  ✓ Rebuild took ${rebuildTime}ms for ${nodes.length} nodes`);
    
    const queryStart = Date.now();
    for (let i = 0; i < 100; i++) {
        const node = nodes[Math.floor(Math.random() * nodes.length)];
        hash.getNearbyForRepulsion(node, 200);
    }
    const queryTime = Date.now() - queryStart;
    
    console.log(`  ✓ 100 queries took ${queryTime}ms (avg ${queryTime/100}ms per query)`);
    console.log('  ✓ Performance test passed\n');
}

console.log('✅ All SpatialHash tests passed!');
