/**
 * Binnacle Graph - Shape Path Functions
 * 
 * Functions for drawing different node shapes on canvas.
 * Each function creates a path that can be filled/stroked.
 */

/**
 * Draw a hexagon path centered at (cx, cy) with the given radius
 * @param {CanvasRenderingContext2D} ctx - Canvas context
 * @param {number} cx - Center X coordinate
 * @param {number} cy - Center Y coordinate
 * @param {number} radius - Hexagon radius
 */
export function drawHexagonPath(ctx, cx, cy, radius) {
    const sides = 6;
    const angleOffset = Math.PI / 6; // Rotate 30 degrees so flat side is at bottom
    ctx.moveTo(
        cx + radius * Math.cos(angleOffset),
        cy + radius * Math.sin(angleOffset)
    );
    for (let i = 1; i <= sides; i++) {
        const angle = angleOffset + (i * 2 * Math.PI / sides);
        ctx.lineTo(
            cx + radius * Math.cos(angle),
            cy + radius * Math.sin(angle)
        );
    }
    ctx.closePath();
}

/**
 * Draw a diamond (rhombus) path centered at (cx, cy) with the given radius
 * @param {CanvasRenderingContext2D} ctx - Canvas context
 * @param {number} cx - Center X coordinate
 * @param {number} cy - Center Y coordinate
 * @param {number} radius - Diamond radius
 */
export function drawDiamondPath(ctx, cx, cy, radius) {
    ctx.moveTo(cx, cy - radius);       // Top
    ctx.lineTo(cx + radius, cy);       // Right
    ctx.lineTo(cx, cy + radius);       // Bottom
    ctx.lineTo(cx - radius, cy);       // Left
    ctx.closePath();
}

/**
 * Draw a robot icon path for agent nodes (rectangular head with antenna + rectangular body)
 * @param {CanvasRenderingContext2D} ctx - Canvas context
 * @param {number} cx - Center X coordinate
 * @param {number} cy - Center Y coordinate
 * @param {number} radius - Overall radius
 */
export function drawRobotPath(ctx, cx, cy, radius) {
    const headWidth = radius * 0.55;
    const headHeight = radius * 0.5;
    const headY = cy - radius * 0.35;
    const bodyWidth = radius * 0.75;
    const bodyHeight = radius * 0.6;
    const bodyY = cy + radius * 0.25;
    const antennaHeight = radius * 0.25;
    const antennaWidth = radius * 0.08;
    
    // Antenna (small rectangle on top of head)
    ctx.rect(cx - antennaWidth / 2, headY - headHeight / 2 - antennaHeight, antennaWidth, antennaHeight);
    
    // Head (rounded rectangle)
    const headCorner = radius * 0.15;
    const headLeft = cx - headWidth / 2;
    const headTop = headY - headHeight / 2;
    ctx.moveTo(headLeft + headCorner, headTop);
    ctx.lineTo(headLeft + headWidth - headCorner, headTop);
    ctx.quadraticCurveTo(headLeft + headWidth, headTop, headLeft + headWidth, headTop + headCorner);
    ctx.lineTo(headLeft + headWidth, headTop + headHeight - headCorner);
    ctx.quadraticCurveTo(headLeft + headWidth, headTop + headHeight, headLeft + headWidth - headCorner, headTop + headHeight);
    ctx.lineTo(headLeft + headCorner, headTop + headHeight);
    ctx.quadraticCurveTo(headLeft, headTop + headHeight, headLeft, headTop + headHeight - headCorner);
    ctx.lineTo(headLeft, headTop + headCorner);
    ctx.quadraticCurveTo(headLeft, headTop, headLeft + headCorner, headTop);
    
    // Body (rounded rectangle)
    const bodyCorner = radius * 0.12;
    const bodyLeft = cx - bodyWidth / 2;
    const bodyTop = bodyY - bodyHeight / 2;
    ctx.moveTo(bodyLeft + bodyCorner, bodyTop);
    ctx.lineTo(bodyLeft + bodyWidth - bodyCorner, bodyTop);
    ctx.quadraticCurveTo(bodyLeft + bodyWidth, bodyTop, bodyLeft + bodyWidth, bodyTop + bodyCorner);
    ctx.lineTo(bodyLeft + bodyWidth, bodyTop + bodyHeight - bodyCorner);
    ctx.quadraticCurveTo(bodyLeft + bodyWidth, bodyTop + bodyHeight, bodyLeft + bodyWidth - bodyCorner, bodyTop + bodyHeight);
    ctx.lineTo(bodyLeft + bodyCorner, bodyTop + bodyHeight);
    ctx.quadraticCurveTo(bodyLeft, bodyTop + bodyHeight, bodyLeft, bodyTop + bodyHeight - bodyCorner);
    ctx.lineTo(bodyLeft, bodyTop + bodyCorner);
    ctx.quadraticCurveTo(bodyLeft, bodyTop, bodyLeft + bodyCorner, bodyTop);
}

/**
 * Draw a square path centered at (cx, cy) with the given radius (half-side length)
 * @param {CanvasRenderingContext2D} ctx - Canvas context
 * @param {number} cx - Center X coordinate
 * @param {number} cy - Center Y coordinate
 * @param {number} radius - Half-side length
 */
export function drawSquarePath(ctx, cx, cy, radius) {
    ctx.moveTo(cx - radius, cy - radius);  // Top-left
    ctx.lineTo(cx + radius, cy - radius);  // Top-right
    ctx.lineTo(cx + radius, cy + radius);  // Bottom-right
    ctx.lineTo(cx - radius, cy + radius);  // Bottom-left
    ctx.closePath();
}

/**
 * Draw a cloud/bubble path centered at (cx, cy) with the given radius
 * Uses bezier curves for smooth transitions between bumps
 * @param {CanvasRenderingContext2D} ctx - Canvas context
 * @param {number} cx - Center X coordinate
 * @param {number} cy - Center Y coordinate
 * @param {number} radius - Cloud radius
 */
export function drawCloudPath(ctx, cx, cy, radius) {
    const w = radius * 1.1;  // Width scale
    const h = radius * 0.75;  // Height scale (slightly flattened)
    
    ctx.moveTo(cx - w, cy + h * 0.3);
    
    // Bottom edge (flat-ish base)
    ctx.quadraticCurveTo(cx - w * 0.5, cy + h * 0.5, cx, cy + h * 0.4);
    ctx.quadraticCurveTo(cx + w * 0.5, cy + h * 0.5, cx + w, cy + h * 0.3);
    
    // Right bump
    ctx.quadraticCurveTo(cx + w * 1.2, cy, cx + w * 0.9, cy - h * 0.3);
    
    // Top-right bump  
    ctx.quadraticCurveTo(cx + w * 0.8, cy - h * 0.8, cx + w * 0.3, cy - h * 0.7);
    
    // Top middle bump (main puffy top)
    ctx.quadraticCurveTo(cx + w * 0.1, cy - h * 1.0, cx - w * 0.2, cy - h * 0.75);
    
    // Top-left bump
    ctx.quadraticCurveTo(cx - w * 0.6, cy - h * 0.9, cx - w * 0.8, cy - h * 0.4);
    
    // Left bump (back to start)
    ctx.quadraticCurveTo(cx - w * 1.2, cy, cx - w, cy + h * 0.3);
    
    ctx.closePath();
}

/**
 * Draw a rounded rectangle path (document/page shape) centered at (cx, cy)
 * Slightly taller than wide to look like a document page
 * @param {CanvasRenderingContext2D} ctx - Canvas context
 * @param {number} cx - Center X coordinate
 * @param {number} cy - Center Y coordinate
 * @param {number} radius - Overall radius
 */
export function drawDocPath(ctx, cx, cy, radius) {
    const w = radius * 0.85;   // Width (narrower than height)
    const h = radius * 1.1;    // Height (taller, page-like)
    const corner = radius * 0.2;  // Corner radius
    const fold = radius * 0.25;   // Corner fold size
    
    // Start at top-left (after corner radius)
    ctx.moveTo(cx - w + corner, cy - h);
    
    // Top edge (until fold corner)
    ctx.lineTo(cx + w - fold, cy - h);
    
    // Folded corner (dog-ear effect)
    ctx.lineTo(cx + w, cy - h + fold);
    
    // Right edge
    ctx.lineTo(cx + w, cy + h - corner);
    
    // Bottom-right corner (rounded)
    ctx.quadraticCurveTo(cx + w, cy + h, cx + w - corner, cy + h);
    
    // Bottom edge
    ctx.lineTo(cx - w + corner, cy + h);
    
    // Bottom-left corner (rounded)
    ctx.quadraticCurveTo(cx - w, cy + h, cx - w, cy + h - corner);
    
    // Left edge
    ctx.lineTo(cx - w, cy - h + corner);
    
    // Top-left corner (rounded)
    ctx.quadraticCurveTo(cx - w, cy - h, cx - w + corner, cy - h);
    
    ctx.closePath();
}

/**
 * Draw the appropriate shape path based on node type
 * @param {CanvasRenderingContext2D} ctx - Canvas context
 * @param {string} nodeType - Node type (task, bug, idea, queue, agent, doc, milestone)
 * @param {number} cx - Center X coordinate
 * @param {number} cy - Center Y coordinate
 * @param {number} radius - Shape radius
 */
export function drawNodeShapePath(ctx, nodeType, cx, cy, radius) {
    switch (nodeType) {
        case 'queue':
            drawHexagonPath(ctx, cx, cy, radius);
            break;
        case 'agent':
            drawRobotPath(ctx, cx, cy, radius);
            break;
        case 'bug':
            drawSquarePath(ctx, cx, cy, radius);
            break;
        case 'idea':
            drawCloudPath(ctx, cx, cy, radius);
            break;
        case 'doc':
            drawDocPath(ctx, cx, cy, radius);
            break;
        case 'task':
        case 'milestone':
        case 'test':
        default:
            // Circle for tasks, milestones, tests, and default
            ctx.arc(cx, cy, radius, 0, Math.PI * 2);
            break;
    }
}
