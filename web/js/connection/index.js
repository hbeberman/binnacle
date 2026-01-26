/**
 * Connection Module Index
 * 
 * Re-exports all connection-related modules for convenient imports.
 * 
 * Usage:
 *   import { connect, disconnect, isConnected } from './connection/index.js';
 *   import { detectMode, initModeDetection } from './connection/index.js';
 *   import { loadArchive, loadArchiveFromFile } from './connection/index.js';
 */

// WebSocket connection
export {
    WebSocketConnection,
    ConnectionState,
    createConnection
} from './websocket.js';

// Live connection (high-level WebSocket + message handling)
export {
    connect,
    disconnect,
    getConnection,
    isConnected,
    getConnectionState,
    send,
    requestSync
} from './live-connection.js';

// Message handlers
export {
    handleMessage,
    registerHandler,
    setReloadCallback,
    getRegisteredTypes
} from './message-handlers.js';

// Mode detection (URL parameter parsing)
export {
    detectMode,
    applyDetectedMode,
    buildConnectionUrl,
    updateBrowserUrl,
    initModeDetection
} from './mode-detection.js';

// Archive loading (WASM-based)
export {
    loadArchive,
    loadArchiveFromFile,
    getViewer,
    getWasmVersion,
    runLayout,
    isLayoutReady,
    isLayoutStable,
    getViewport,
    setViewport,
    focusNode,
    findNodeAt,
    dispose
} from './archive.js';
