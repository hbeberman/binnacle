/**
 * Touch Handler Utility
 * 
 * Provides mobile-friendly touch interactions:
 * - Long-press detection for node selection
 * - Touch-based multi-select support
 */

const LONG_PRESS_DURATION = 500; // milliseconds

/**
 * Add long-press detection to an element
 * @param {HTMLElement} element - Element to attach handler to
 * @param {Function} onLongPress - Callback when long-press detected
 * @param {Function} onClick - Optional callback for regular click
 * @returns {Function} Cleanup function to remove listeners
 */
export function addLongPressHandler(element, onLongPress, onClick = null) {
    let pressTimer = null;
    let touchStartPos = null;
    let longPressTriggered = false;
    
    const MOVE_THRESHOLD = 10; // pixels - cancel if finger moves too much
    
    function handleTouchStart(e) {
        longPressTriggered = false;
        const touch = e.touches[0];
        touchStartPos = { x: touch.clientX, y: touch.clientY };
        
        pressTimer = setTimeout(() => {
            longPressTriggered = true;
            // Add haptic feedback if available
            if (navigator.vibrate) {
                navigator.vibrate(50);
            }
            onLongPress(e);
        }, LONG_PRESS_DURATION);
    }
    
    function handleTouchMove(e) {
        if (!touchStartPos || !pressTimer) return;
        
        const touch = e.touches[0];
        const dx = touch.clientX - touchStartPos.x;
        const dy = touch.clientY - touchStartPos.y;
        const distance = Math.sqrt(dx * dx + dy * dy);
        
        // Cancel long-press if finger moved too much
        if (distance > MOVE_THRESHOLD) {
            clearTimeout(pressTimer);
            pressTimer = null;
        }
    }
    
    function handleTouchEnd(e) {
        if (pressTimer) {
            clearTimeout(pressTimer);
            pressTimer = null;
        }
        
        // If long-press was triggered, don't fire click
        if (longPressTriggered) {
            e.preventDefault();
            return;
        }
        
        // Fire click handler if provided and this was a short tap
        if (onClick && !longPressTriggered) {
            onClick(e);
        }
    }
    
    function handleTouchCancel(e) {
        if (pressTimer) {
            clearTimeout(pressTimer);
            pressTimer = null;
        }
        longPressTriggered = false;
    }
    
    // Attach listeners
    element.addEventListener('touchstart', handleTouchStart, { passive: false });
    element.addEventListener('touchmove', handleTouchMove, { passive: false });
    element.addEventListener('touchend', handleTouchEnd, { passive: false });
    element.addEventListener('touchcancel', handleTouchCancel, { passive: false });
    
    // Return cleanup function
    return () => {
        element.removeEventListener('touchstart', handleTouchStart);
        element.removeEventListener('touchmove', handleTouchMove);
        element.removeEventListener('touchend', handleTouchEnd);
        element.removeEventListener('touchcancel', handleTouchCancel);
        if (pressTimer) {
            clearTimeout(pressTimer);
        }
    };
}

/**
 * Add long-press multi-select to canvas-based node rendering
 * @param {HTMLCanvasElement} canvas - Canvas element
 * @param {Function} getNodeAtPosition - Function to get node at x,y coordinates
 * @param {Function} onNodeLongPress - Callback when node is long-pressed
 * @returns {Function} Cleanup function
 */
export function addCanvasLongPress(canvas, getNodeAtPosition, onNodeLongPress) {
    let pressTimer = null;
    let touchStartPos = null;
    let touchStartNode = null;
    let longPressTriggered = false;
    
    const MOVE_THRESHOLD = 10;
    
    function handleTouchStart(e) {
        longPressTriggered = false;
        const touch = e.touches[0];
        const rect = canvas.getBoundingClientRect();
        const x = touch.clientX - rect.left;
        const y = touch.clientY - rect.top;
        
        touchStartPos = { x: touch.clientX, y: touch.clientY };
        touchStartNode = getNodeAtPosition(x, y);
        
        if (touchStartNode) {
            pressTimer = setTimeout(() => {
                longPressTriggered = true;
                if (navigator.vibrate) {
                    navigator.vibrate(50);
                }
                onNodeLongPress(touchStartNode, e);
            }, LONG_PRESS_DURATION);
        }
    }
    
    function handleTouchMove(e) {
        if (!touchStartPos || !pressTimer) return;
        
        const touch = e.touches[0];
        const dx = touch.clientX - touchStartPos.x;
        const dy = touch.clientY - touchStartPos.y;
        const distance = Math.sqrt(dx * dx + dy * dy);
        
        if (distance > MOVE_THRESHOLD) {
            clearTimeout(pressTimer);
            pressTimer = null;
            touchStartNode = null;
        }
    }
    
    function handleTouchEnd(e) {
        if (pressTimer) {
            clearTimeout(pressTimer);
            pressTimer = null;
        }
        
        if (longPressTriggered) {
            e.preventDefault();
        }
        
        touchStartNode = null;
        touchStartPos = null;
    }
    
    function handleTouchCancel(e) {
        if (pressTimer) {
            clearTimeout(pressTimer);
            pressTimer = null;
        }
        longPressTriggered = false;
        touchStartNode = null;
        touchStartPos = null;
    }
    
    canvas.addEventListener('touchstart', handleTouchStart, { passive: false });
    canvas.addEventListener('touchmove', handleTouchMove, { passive: false });
    canvas.addEventListener('touchend', handleTouchEnd, { passive: false });
    canvas.addEventListener('touchcancel', handleTouchCancel, { passive: false });
    
    return () => {
        canvas.removeEventListener('touchstart', handleTouchStart);
        canvas.removeEventListener('touchmove', handleTouchMove);
        canvas.removeEventListener('touchend', handleTouchEnd);
        canvas.removeEventListener('touchcancel', handleTouchCancel);
        if (pressTimer) {
            clearTimeout(pressTimer);
        }
    };
}
