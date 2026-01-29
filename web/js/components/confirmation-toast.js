/**
 * Confirmation Toast Component
 * 
 * Displays a modal confirmation dialog with a message and Confirm/Cancel buttons.
 * Used for confirming potentially expensive operations like revealing large families.
 */

let activeToast = null;

/**
 * Show a confirmation toast
 * @param {string} message - Message to display
 * @param {Object} callbacks - Callback functions
 * @param {Function} callbacks.onConfirm - Called when user confirms
 * @param {Function} callbacks.onCancel - Called when user cancels (optional)
 * @returns {HTMLElement} The toast overlay element
 */
export function showConfirmationToast(message, { onConfirm, onCancel }) {
    // Remove any existing toast
    if (activeToast) {
        activeToast.remove();
        activeToast = null;
    }
    
    // Create overlay
    const overlay = document.createElement('div');
    overlay.className = 'confirmation-toast-overlay';
    
    // Create toast
    const toast = document.createElement('div');
    toast.className = 'confirmation-toast';
    
    // Message
    const messageEl = document.createElement('div');
    messageEl.className = 'confirmation-toast-message';
    messageEl.textContent = message;
    toast.appendChild(messageEl);
    
    // Actions container
    const actions = document.createElement('div');
    actions.className = 'confirmation-toast-actions';
    
    // Cancel button
    const cancelBtn = document.createElement('button');
    cancelBtn.className = 'confirmation-toast-btn';
    cancelBtn.textContent = 'Cancel';
    cancelBtn.addEventListener('click', () => {
        hideToast();
        if (onCancel) {
            onCancel();
        }
    });
    actions.appendChild(cancelBtn);
    
    // Confirm button
    const confirmBtn = document.createElement('button');
    confirmBtn.className = 'confirmation-toast-btn primary';
    confirmBtn.textContent = 'Reveal';
    confirmBtn.addEventListener('click', () => {
        hideToast();
        onConfirm();
    });
    actions.appendChild(confirmBtn);
    
    toast.appendChild(actions);
    overlay.appendChild(toast);
    
    // Add to DOM
    document.body.appendChild(overlay);
    activeToast = overlay;
    
    // Close on overlay click
    overlay.addEventListener('click', (e) => {
        if (e.target === overlay) {
            hideToast();
            if (onCancel) {
                onCancel();
            }
        }
    });
    
    // Close on Escape key
    const escapeHandler = (e) => {
        if (e.key === 'Escape') {
            hideToast();
            if (onCancel) {
                onCancel();
            }
            document.removeEventListener('keydown', escapeHandler);
        }
    };
    document.addEventListener('keydown', escapeHandler);
    
    function hideToast() {
        if (overlay && overlay.parentNode) {
            overlay.classList.add('hidden');
            setTimeout(() => {
                overlay.remove();
            }, 200);
        }
        if (activeToast === overlay) {
            activeToast = null;
        }
    }
    
    return overlay;
}

/**
 * Hide any active confirmation toast
 */
export function hideConfirmationToast() {
    if (activeToast) {
        activeToast.classList.add('hidden');
        setTimeout(() => {
            if (activeToast && activeToast.parentNode) {
                activeToast.remove();
            }
            activeToast = null;
        }, 200);
    }
}
