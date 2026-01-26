/**
 * URL Parameter Mode Detection
 * 
 * Parses URL parameters to determine connection mode:
 * - ?ws=HOST:PORT → WebSocket live mode
 * - ?archive=URL → Archive mode (readonly)
 * - No params → Show connection picker
 * 
 * Also handles fragment anchors for node focus:
 * - #bn-xxxx → Focus on entity after connection
 */

import { ConnectionMode, setMode } from '../state.js';

/**
 * Connection mode detection result
 * @typedef {Object} ModeDetectionResult
 * @property {string} mode - The detected connection mode (from ConnectionMode enum)
 * @property {string|null} wsUrl - WebSocket URL if mode is WEBSOCKET
 * @property {string|null} archiveUrl - Archive URL if mode is ARCHIVE
 * @property {string|null} focusEntityId - Entity ID to focus on from URL fragment
 */

/**
 * Parse URL parameters and hash fragment to detect connection mode
 * @param {string} [url] - URL to parse (defaults to current window location)
 * @returns {ModeDetectionResult} Detection result
 */
export function detectMode(url) {
    let urlObj;
    if (url) {
        // URL provided - parse it (use dummy base for relative URLs)
        urlObj = new URL(url, 'https://localhost/');
    } else if (typeof window !== 'undefined') {
        // Browser environment - use current location
        urlObj = new URL(window.location.href);
    } else {
        // Node.js environment with no URL - return default
        return {
            mode: ConnectionMode.NONE,
            wsUrl: null,
            archiveUrl: null,
            focusEntityId: null
        };
    }
    const params = urlObj.searchParams;
    const hash = urlObj.hash;
    
    // Parse focus entity from hash fragment (e.g., #bn-a1b2)
    const focusEntityId = parseFocusEntity(hash);
    
    // Check for archive URL parameter
    const archiveParam = params.get('archive');
    if (archiveParam) {
        const archiveUrl = normalizeArchiveUrl(archiveParam);
        return {
            mode: ConnectionMode.ARCHIVE,
            wsUrl: null,
            archiveUrl,
            focusEntityId
        };
    }
    
    // Check for WebSocket URL parameter
    const wsParam = params.get('ws');
    if (wsParam) {
        const wsUrl = normalizeWebSocketUrl(wsParam);
        return {
            mode: ConnectionMode.WEBSOCKET,
            wsUrl,
            archiveUrl: null,
            focusEntityId
        };
    }
    
    // No parameters - show picker
    return {
        mode: ConnectionMode.NONE,
        wsUrl: null,
        archiveUrl: null,
        focusEntityId
    };
}

/**
 * Parse focus entity ID from URL hash fragment
 * Supports formats: #bn-xxxx, #bnt-xxxx, #bnq-xxxx, #bne-xxxx
 * @param {string} hash - URL hash (including # prefix)
 * @returns {string|null} Entity ID or null if not a valid entity reference
 */
function parseFocusEntity(hash) {
    if (!hash || hash.length < 2) {
        return null;
    }
    
    // Remove # prefix
    const fragment = hash.slice(1);
    
    // Validate entity ID format (bn-xxxx, bnt-xxxx, bnq-xxxx, bne-xxxx)
    // Entity IDs are typically 4 hex characters after the prefix
    const entityPattern = /^(bn|bnt|bnq|bne)-[a-f0-9]{4}$/i;
    
    if (entityPattern.test(fragment)) {
        return fragment.toLowerCase();
    }
    
    return null;
}

/**
 * Normalize WebSocket URL from various input formats
 * Accepts: HOST:PORT, ws://HOST:PORT, wss://HOST:PORT, localhost:PORT
 * @param {string} input - Raw WebSocket URL/address input
 * @returns {string} Normalized WebSocket URL (ws:// or wss://)
 */
function normalizeWebSocketUrl(input) {
    let url = input.trim();
    
    // Already has protocol
    if (url.startsWith('ws://') || url.startsWith('wss://')) {
        // Ensure it has /ws path if not already present
        const parsed = new URL(url);
        if (!parsed.pathname || parsed.pathname === '/') {
            parsed.pathname = '/ws';
        }
        return parsed.toString();
    }
    
    // Handle HOST:PORT format
    // Use ws:// for localhost, wss:// for other hosts by default
    const isLocalhost = url.startsWith('localhost') || 
                        url.startsWith('127.0.0.1') ||
                        url.startsWith('[::1]');
    
    const protocol = isLocalhost ? 'ws://' : 'wss://';
    const fullUrl = new URL(`${protocol}${url}`);
    
    // Ensure /ws path
    if (!fullUrl.pathname || fullUrl.pathname === '/') {
        fullUrl.pathname = '/ws';
    }
    
    return fullUrl.toString();
}

/**
 * Normalize archive URL
 * Handles relative URLs, ensures proper encoding
 * @param {string} input - Raw archive URL input
 * @returns {string} Normalized archive URL
 */
function normalizeArchiveUrl(input) {
    let url = input.trim();
    
    // Handle relative URLs by making them absolute
    if (!url.startsWith('http://') && !url.startsWith('https://') && !url.startsWith('file://')) {
        // Relative URL - resolve against current location (browser only)
        if (typeof window !== 'undefined') {
            return new URL(url, window.location.href).toString();
        }
        // In non-browser environment, return as-is (caller should provide absolute URL)
        return url;
    }
    
    return url;
}

/**
 * Apply detected mode to global state
 * @param {ModeDetectionResult} detection - Detection result
 */
export function applyDetectedMode(detection) {
    const options = {};
    
    if (detection.wsUrl) {
        options.wsUrl = detection.wsUrl;
    }
    if (detection.archiveUrl) {
        options.archiveUrl = detection.archiveUrl;
    }
    
    setMode(detection.mode, options);
}

/**
 * Build a URL with connection parameters
 * Useful for generating shareable links
 * @param {Object} options - URL options
 * @param {string} [options.wsUrl] - WebSocket URL for live mode
 * @param {string} [options.archiveUrl] - Archive URL for archive mode
 * @param {string} [options.focusEntityId] - Entity ID for focus
 * @param {string} [options.baseUrl] - Base URL (defaults to current page)
 * @returns {string} Complete URL with parameters
 */
export function buildConnectionUrl(options = {}) {
    let baseUrl = options.baseUrl;
    if (!baseUrl && typeof window !== 'undefined') {
        baseUrl = window.location.origin + window.location.pathname;
    }
    if (!baseUrl) {
        baseUrl = 'https://localhost/';
    }
    const url = new URL(baseUrl);
    
    // Add connection parameter
    if (options.wsUrl) {
        // Store simplified version (strip ws:// and /ws for cleaner URLs)
        let wsParam = options.wsUrl;
        if (wsParam.startsWith('ws://')) {
            wsParam = wsParam.slice(5);
        }
        if (wsParam.endsWith('/ws')) {
            wsParam = wsParam.slice(0, -3);
        }
        url.searchParams.set('ws', wsParam);
    } else if (options.archiveUrl) {
        url.searchParams.set('archive', options.archiveUrl);
    }
    
    // Add focus fragment
    if (options.focusEntityId) {
        url.hash = options.focusEntityId;
    }
    
    return url.toString();
}

/**
 * Update URL in browser without page reload
 * Used when connection is established to update shareable URL
 * @param {Object} options - URL options (same as buildConnectionUrl)
 */
export function updateBrowserUrl(options = {}) {
    if (typeof window === 'undefined') {
        return; // Not in browser environment
    }
    const newUrl = buildConnectionUrl(options);
    window.history.replaceState({}, '', newUrl);
}

/**
 * Initialize mode detection and route to appropriate flow
 * This is the main entry point for the mode detection system
 * @param {Object} callbacks - Callback functions for each mode
 * @param {Function} [callbacks.onWebSocket] - Called when WebSocket mode detected (receives wsUrl, focusEntityId)
 * @param {Function} [callbacks.onArchive] - Called when Archive mode detected (receives archiveUrl, focusEntityId)
 * @param {Function} [callbacks.onPicker] - Called when no mode detected (show picker)
 * @returns {ModeDetectionResult} Detection result
 */
export function initModeDetection(callbacks = {}) {
    const detection = detectMode();
    
    // Apply mode to state
    applyDetectedMode(detection);
    
    // Route to appropriate callback
    switch (detection.mode) {
        case ConnectionMode.WEBSOCKET:
            if (callbacks.onWebSocket) {
                callbacks.onWebSocket(detection.wsUrl, detection.focusEntityId);
            }
            break;
            
        case ConnectionMode.ARCHIVE:
            if (callbacks.onArchive) {
                callbacks.onArchive(detection.archiveUrl, detection.focusEntityId);
            }
            break;
            
        case ConnectionMode.NONE:
        default:
            if (callbacks.onPicker) {
                callbacks.onPicker(detection.focusEntityId);
            }
            break;
    }
    
    return detection;
}

// Export for testing
export { parseFocusEntity, normalizeWebSocketUrl, normalizeArchiveUrl };
