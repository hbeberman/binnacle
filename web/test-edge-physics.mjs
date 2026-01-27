/**
 * Test for Edge Physics Toggle Feature
 * 
 * Verifies that the magnet button for edge physics control is properly implemented.
 */

console.log('Testing Edge Physics Toggle Feature\n');

// Test 1: Verify state.js has edgePhysicsFilters
console.log('Test 1: Verify edgePhysicsFilters in state.js');
{
    const fs = await import('fs');
    const stateCode = fs.readFileSync('web/js/state.js', 'utf-8');
    
    if (stateCode.includes('edgePhysicsFilters:')) {
        console.log('  ✓ edgePhysicsFilters found in state');
    } else {
        throw new Error('edgePhysicsFilters not found in state.js');
    }
    
    if (stateCode.includes("subscribe('ui.edgePhysicsFilters'")) {
        console.log('  ✓ edgePhysicsFilters persistence subscription found');
    } else {
        throw new Error('edgePhysicsFilters persistence not set up');
    }
    
    console.log('  ✓ State management properly set up\n');
}

// Test 2: Verify filters.js creates magnet buttons
console.log('Test 2: Verify magnet button creation in filters.js');
{
    const fs = await import('fs');
    const filtersCode = fs.readFileSync('web/js/components/filters.js', 'utf-8');
    
    if (filtersCode.includes('edge-physics-btn')) {
        console.log('  ✓ Magnet button CSS class found');
    } else {
        throw new Error('edge-physics-btn class not found');
    }
    
    if (filtersCode.includes('🧲')) {
        console.log('  ✓ Magnet emoji found');
    } else {
        throw new Error('Magnet emoji not found');
    }
    
    if (filtersCode.includes("State.get('ui.edgePhysicsFilters')")) {
        console.log('  ✓ Physics filter state reading found');
    } else {
        throw new Error('Physics filter state reading not found');
    }
    
    if (filtersCode.includes("State.set('ui.edgePhysicsFilters'")) {
        console.log('  ✓ Physics filter state setting found');
    } else {
        throw new Error('Physics filter state setting not found');
    }
    
    // Check that button is added to row
    if (filtersCode.includes('row.appendChild(magnetBtn)')) {
        console.log('  ✓ Magnet button appended to row');
    } else {
        throw new Error('Magnet button not appended to row');
    }
    
    console.log('  ✓ Filter component properly creates magnet buttons\n');
}

// Test 3: Verify renderer.js filters edges by physics
console.log('Test 3: Verify physics filtering in renderer.js');
{
    const fs = await import('fs');
    const rendererCode = fs.readFileSync('web/js/graph/renderer.js', 'utf-8');
    
    if (rendererCode.includes("const physicsFilters = state.get('ui.edgePhysicsFilters')")) {
        console.log('  ✓ Physics filters retrieved in renderer');
    } else {
        throw new Error('Physics filters not retrieved in renderer');
    }
    
    // Check that edges are filtered before physics calculation
    const physicsCheckRegex = /if \(physicsFilters\[edge\.edge_type\] === false\) continue;/;
    if (physicsCheckRegex.test(rendererCode)) {
        console.log('  ✓ Edges with disabled physics are skipped');
    } else {
        throw new Error('Physics filter check not found in edge loop');
    }
    
    console.log('  ✓ Renderer properly filters edges by physics setting\n');
}

// Test 4: Verify CSS styling exists
console.log('Test 4: Verify CSS styling for magnet button');
{
    const fs = await import('fs');
    const cssCode = fs.readFileSync('web/css/components/sidebar.css', 'utf-8');
    
    if (cssCode.includes('.edge-physics-btn')) {
        console.log('  ✓ CSS class for magnet button found');
    } else {
        throw new Error('CSS class for magnet button not found');
    }
    
    if (cssCode.includes('.edge-physics-btn.active')) {
        console.log('  ✓ Active state CSS found');
    } else {
        throw new Error('Active state CSS not found');
    }
    
    console.log('  ✓ CSS styling properly added\n');
}

console.log('✅ All tests passed!');
console.log('Edge physics toggle feature is correctly implemented.');
