/**
 * Progressive Reveal Animation
 * 
 * Animates family reveal in waves by depth level with fade-in effects.
 * Nodes appear progressively (~75ms between waves) for smooth visual flow.
 */

// Animation constants
const WAVE_DELAY_MS = 75; // Delay between depth levels
const FADE_IN_DURATION_MS = 300; // Duration of fade-in for each node

// Track active reveal animations: Map<nodeId, startTime>
const revealAnimations = new Map();

/**
 * Animate family reveal progressively by depth
 * @param {string} rootId - Root node ID
 * @param {Set<string>} descendants - All descendant node IDs to reveal
 * @param {Map<string, number>} depthMap - Map of node ID to depth from root
 * @param {Map<string, {x: number, y: number}>} existingNodes - Currently visible nodes
 * @returns {Promise<void>} Resolves when animation completes
 */
export async function animateProgressiveReveal(rootId, descendants, depthMap, existingNodes) {
    // Find max depth to determine how many waves we need
    const maxDepth = Math.max(...Array.from(depthMap.values()));
    
    console.log(`[RevealAnimation] Starting progressive reveal: ${descendants.size} nodes, ${maxDepth + 1} depth levels`);
    
    // Reveal nodes in waves by depth
    for (let depth = 0; depth <= maxDepth; depth++) {
        // Get all nodes at this depth
        const nodesAtDepth = Array.from(descendants).filter(id => depthMap.get(id) === depth);
        
        if (nodesAtDepth.length === 0) {
            continue;
        }
        
        console.log(`[RevealAnimation] Revealing depth ${depth}: ${nodesAtDepth.length} nodes`);
        
        // Start fade-in animation for newly revealed nodes at this depth
        const now = performance.now();
        for (const nodeId of nodesAtDepth) {
            // Only animate nodes that weren't already visible
            if (!existingNodes.has(nodeId)) {
                revealAnimations.set(nodeId, now);
            }
        }
        
        // Wait before revealing next wave (except after the last wave)
        if (depth < maxDepth) {
            await sleep(WAVE_DELAY_MS);
        }
    }
    
    console.log('[RevealAnimation] Progressive reveal complete');
}

/**
 * Get current opacity for a node during reveal animation
 * @param {string} nodeId - Node ID
 * @returns {number} Opacity value 0.0-1.0, or null if no active animation
 */
export function getRevealOpacity(nodeId) {
    const startTime = revealAnimations.get(nodeId);
    if (!startTime) {
        return null; // No active animation for this node
    }
    
    const elapsed = performance.now() - startTime;
    if (elapsed >= FADE_IN_DURATION_MS) {
        // Animation complete, remove from tracking
        revealAnimations.delete(nodeId);
        return null;
    }
    
    // Ease-out fade in: starts at 0, smoothly reaches 1.0
    const progress = elapsed / FADE_IN_DURATION_MS;
    return easeOutQuad(progress);
}

/**
 * Check if any reveal animations are active
 * @returns {boolean}
 */
export function hasActiveRevealAnimations() {
    return revealAnimations.size > 0;
}

/**
 * Clear all reveal animations (e.g., when collapsing family)
 */
export function clearRevealAnimations() {
    revealAnimations.clear();
}

/**
 * Ease-out quadratic - starts fast, decelerates
 * Good for fade-in effects
 * @param {number} t - Progress 0.0-1.0
 * @returns {number} Eased value
 */
function easeOutQuad(t) {
    return t * (2 - t);
}

/**
 * Sleep for specified milliseconds
 * @param {number} ms - Milliseconds to sleep
 * @returns {Promise<void>}
 */
function sleep(ms) {
    return new Promise(resolve => setTimeout(resolve, ms));
}
