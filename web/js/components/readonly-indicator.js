/**
 * Readonly Indicator Component
 * 
 * Manages UI state for readonly/archive mode:
 * - Displays header badge indicating readonly status
 * - Applies disabled styles to write-action buttons
 * - Shows tooltips explaining why actions are disabled
 */

import { 
    subscribe, 
    isReadonly, 
    getMode, 
    ConnectionMode 
} from '../state.js';

// Default tooltip message for disabled actions
const DEFAULT_READONLY_TOOLTIP = 'Action unavailable in readonly mode';
const ARCHIVE_READONLY_TOOLTIP = 'Action unavailable when viewing archive';

/**
 * Create the readonly badge HTML element
 * @returns {HTMLElement} The badge element
 */
function createReadonlyBadge() {
    const badge = document.createElement('div');
    badge.className = 'readonly-badge';
    badge.id = 'readonly-badge';
    
    badge.innerHTML = `
        <span class="readonly-badge-icon">🔒</span>
        <span class="readonly-badge-text">Read Only</span>
    `;
    
    return badge;
}

/**
 * Update the badge based on current mode
 * @param {HTMLElement} badge - The badge element
 * @param {string} mode - Current connection mode
 */
function updateBadgeForMode(badge, mode) {
    if (mode === ConnectionMode.ARCHIVE) {
        badge.classList.add('archive-mode');
        badge.querySelector('.readonly-badge-icon').textContent = '📦';
        badge.querySelector('.readonly-badge-text').textContent = 'Archive Mode';
    } else {
        badge.classList.remove('archive-mode');
        badge.querySelector('.readonly-badge-icon').textContent = '🔒';
        badge.querySelector('.readonly-badge-text').textContent = 'Read Only';
    }
}

/**
 * Apply or remove readonly state to the document
 * @param {boolean} readonly - Whether readonly mode is active
 */
function applyReadonlyState(readonly) {
    if (readonly) {
        document.body.classList.add('readonly-mode');
    } else {
        document.body.classList.remove('readonly-mode');
    }
}

/**
 * Get appropriate tooltip text based on mode
 * @param {string} mode - Current connection mode
 * @returns {string} Tooltip text
 */
function getTooltipText(mode) {
    if (mode === ConnectionMode.ARCHIVE) {
        return ARCHIVE_READONLY_TOOLTIP;
    }
    return DEFAULT_READONLY_TOOLTIP;
}

/**
 * Update all write-action container tooltips
 * @param {string} mode - Current connection mode
 */
function updateWriteActionTooltips(mode) {
    const tooltipText = getTooltipText(mode);
    const containers = document.querySelectorAll('.write-action-container');
    
    containers.forEach(container => {
        container.setAttribute('data-readonly-tooltip', tooltipText);
    });
}

/**
 * Create a readonly notice element for info panels
 * @param {string} mode - Current connection mode
 * @returns {HTMLElement} Notice element
 */
export function createReadonlyNotice(mode = null) {
    const notice = document.createElement('div');
    notice.className = 'info-panel-readonly-notice';
    notice.id = 'readonly-notice';
    
    const currentMode = mode || getMode();
    const isArchive = currentMode === ConnectionMode.ARCHIVE;
    
    notice.innerHTML = `
        <span class="info-panel-readonly-notice-icon">${isArchive ? '📦' : '🔒'}</span>
        <span>${isArchive ? 'Viewing archive - changes cannot be made' : 'Readonly mode - changes cannot be made'}</span>
    `;
    
    return notice;
}

/**
 * Mount the readonly indicator to a header element
 * @param {HTMLElement|string} headerTarget - Header element or selector
 * @returns {Object} Controller with show/hide methods
 */
export function mountReadonlyIndicator(headerTarget) {
    const header = typeof headerTarget === 'string'
        ? document.querySelector(headerTarget)
        : headerTarget;
    
    if (!header) {
        console.warn('Readonly indicator: header target not found');
        return null;
    }
    
    // Create badge but don't insert yet
    const badge = createReadonlyBadge();
    let isInserted = false;
    
    const controller = {
        /**
         * Show the readonly badge
         */
        show() {
            if (!isInserted) {
                // Insert at the beginning of the header
                header.insertBefore(badge, header.firstChild);
                isInserted = true;
            }
            badge.classList.remove('hidden');
            applyReadonlyState(true);
            updateBadgeForMode(badge, getMode());
            updateWriteActionTooltips(getMode());
        },
        
        /**
         * Hide the readonly badge
         */
        hide() {
            badge.classList.add('hidden');
            applyReadonlyState(false);
        },
        
        /**
         * Update badge for current mode
         */
        update() {
            if (isReadonly()) {
                this.show();
            } else {
                this.hide();
            }
        },
        
        /**
         * Get the badge element
         * @returns {HTMLElement}
         */
        getElement() {
            return badge;
        }
    };
    
    // Subscribe to state changes
    subscribe('readonly', (readonly) => {
        controller.update();
    });
    
    subscribe('mode', (mode) => {
        if (isReadonly()) {
            updateBadgeForMode(badge, mode);
            updateWriteActionTooltips(mode);
        }
    });
    
    // Initialize based on current state
    controller.update();
    
    return controller;
}

/**
 * Mark an element as a write action (will be disabled in readonly mode)
 * @param {HTMLElement} element - The element to mark
 * @param {string} [customTooltip] - Optional custom tooltip message
 */
export function markAsWriteAction(element, customTooltip = null) {
    element.classList.add('write-action');
    
    // If element should show a tooltip, wrap in a container
    if (customTooltip || element.parentElement) {
        const container = element.closest('.write-action-container');
        if (!container && element.parentElement) {
            // Create wrapper if not already wrapped
            const wrapper = document.createElement('div');
            wrapper.className = 'write-action-container';
            wrapper.setAttribute('data-readonly-tooltip', customTooltip || getTooltipText(getMode()));
            element.parentElement.insertBefore(wrapper, element);
            wrapper.appendChild(element);
        } else if (container && customTooltip) {
            container.setAttribute('data-readonly-tooltip', customTooltip);
        }
    }
}

/**
 * Check if an action should be allowed based on readonly state
 * @returns {boolean} True if action is allowed
 */
export function isActionAllowed() {
    return !isReadonly();
}

/**
 * Guard a function to only execute if not in readonly mode
 * @param {Function} fn - Function to guard
 * @param {Function} [onBlocked] - Optional callback when action is blocked
 * @returns {Function} Guarded function
 */
export function guardWriteAction(fn, onBlocked = null) {
    return function(...args) {
        if (isReadonly()) {
            if (onBlocked) {
                onBlocked(getMode());
            }
            return;
        }
        return fn.apply(this, args);
    };
}

/**
 * Create a connection mode indicator element
 * @returns {HTMLElement} The indicator element
 */
export function createConnectionModeIndicator() {
    const indicator = document.createElement('div');
    indicator.className = 'connection-mode-indicator';
    indicator.id = 'connection-mode-indicator';
    
    indicator.innerHTML = `
        <span class="connection-mode-dot"></span>
        <span class="connection-mode-text">Disconnected</span>
    `;
    
    // Subscribe to mode changes
    subscribe('mode', (mode) => {
        updateConnectionModeIndicator(indicator, mode);
    });
    
    // Initialize
    updateConnectionModeIndicator(indicator, getMode());
    
    return indicator;
}

/**
 * Update connection mode indicator display
 * @param {HTMLElement} indicator - The indicator element
 * @param {string} mode - Current connection mode
 */
function updateConnectionModeIndicator(indicator, mode) {
    indicator.classList.remove('live', 'archive');
    
    const textEl = indicator.querySelector('.connection-mode-text');
    
    switch (mode) {
        case ConnectionMode.WEBSOCKET:
            indicator.classList.add('live');
            textEl.textContent = 'Live';
            break;
        case ConnectionMode.ARCHIVE:
            indicator.classList.add('archive');
            textEl.textContent = 'Archive';
            break;
        default:
            textEl.textContent = 'Disconnected';
    }
}
