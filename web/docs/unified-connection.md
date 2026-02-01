# Unified Connection Architecture

The unified connection architecture provides a consistent interface for connecting to different binnacle data sources:

- **LiveConnection**: Real-time WebSocket connection to `bn gui` server
- **ArchiveConnection**: Read-only WASM-based viewer for `.bng` archive files
- **HostedConnection**: (Future) Hosted/CDN mode for static deployments

## Quick Start

### Live Mode (WebSocket)

Connect to a running `bn gui` server:

```javascript
import { createConnection } from './js/connection/unified-connection.js';

const conn = createConnection({
    mode: 'live',
    wsUrl: 'ws://localhost:55823/ws'
});

conn.on('connected', () => {
    console.log('Connected to binnacle server');
});

conn.on('stateChange', (changeType) => {
    console.log('State updated:', changeType);
});

await conn.connect();
```

### Archive Mode (Read-only)

Load a `.bng` archive file:

```javascript
import { createConnection } from './js/connection/unified-connection.js';

const conn = createConnection({
    mode: 'archive',
    archiveUrl: 'https://example.com/project-snapshot.bng'
});

conn.on('connected', (archiveInfo) => {
    console.log(`Loaded archive: ${archiveInfo.nodeCount} nodes, ${archiveInfo.edgeCount} edges`);
});

await conn.connect();
```

### Auto-detect Mode

Let the factory function detect the mode based on provided parameters:

```javascript
// Automatically uses LiveConnection
const liveConn = createConnection({
    wsUrl: 'ws://localhost:55823/ws'
});

// Automatically uses ArchiveConnection
const archiveConn = createConnection({
    archiveUrl: 'project.bng'
});
```

## Connection Interface

All connection adapters implement this interface:

### Methods

- **`connect()`**: Establish connection
- **`disconnect()`**: Close connection
- **`isConnected()`**: Check if currently connected
- **`isReadonly()`**: Check if connection is read-only
- **`send(data)`**: Send data (only for writable connections)
- **`requestSync()`**: Request full state sync (only for live connections)

### Events

- **`connected`**: Fired when connection is established
- **`disconnected`**: Fired when connection is lost
- **`error`**: Fired on connection error
- **`stateChange`**: Fired when data changes (with change type)
- **`statusChange`**: Fired when connection status changes
- **`layoutProgress`**: (Archive only) Fired during layout computation

### Example

```javascript
const conn = createConnection({ wsUrl: 'ws://localhost:55823/ws' });

conn.on('connected', () => {
    console.log('Connection established');
});

conn.on('disconnected', () => {
    console.log('Connection lost');
});

conn.on('error', (error) => {
    console.error('Connection error:', error);
});

conn.on('stateChange', (changeType) => {
    console.log('Data updated:', changeType);
    // Refresh UI with new data from state
});

await conn.connect();

// Later...
await conn.disconnect();
```

## Status Badge

The connection status badge automatically adapts to the connection mode:

- **🟢 Connected**: Live WebSocket connection active
- **🔴 Disconnected**: No connection
- **📦 Archive**: Archive mode (read-only)
- **⏳ Loading**: Connecting or loading archive

The badge is implemented in `js/components/connection-status.js` and updates automatically when the connection mode or status changes.

## Read-only Mode

Archive connections are always read-only. When in archive mode:

- The global state sets `readonly: true`
- Write operations (create/update/delete) are disabled in the UI
- The status badge shows 📦 Archive
- All data is loaded from the archive file

You can check if the current connection is read-only:

```javascript
if (conn.isReadonly()) {
    console.log('Read-only mode - write operations disabled');
}
```

## Advanced Usage

### File Upload (Archive)

Load an archive from a File object (drag-drop or file input):

```javascript
const fileInput = document.querySelector('input[type="file"]');
fileInput.addEventListener('change', async (e) => {
    const file = e.target.files[0];
    
    const conn = createConnection({
        mode: 'archive',
        archiveFile: file
    });
    
    await conn.connect();
});
```

### Custom Event Handling

```javascript
const conn = createConnection({ wsUrl: 'ws://localhost:55823/ws' });

// Multiple listeners per event
conn.on('stateChange', updateGraph);
conn.on('stateChange', updateSidebar);
conn.on('stateChange', updateActivityLog);

await conn.connect();

// Remove listener when component unmounts
conn.off('stateChange', updateGraph);
```

## Migration Guide

### Before (Direct live-connection.js usage)

```javascript
import * as connection from './js/connection/live-connection.js';

await connection.connect('ws://localhost:55823/ws', {
    onConnected: () => { ... },
    onStateChange: () => { ... }
});
```

### After (Unified connection)

```javascript
import { createConnection } from './js/connection/unified-connection.js';

const conn = createConnection({ wsUrl: 'ws://localhost:55823/ws' });

conn.on('connected', () => { ... });
conn.on('stateChange', () => { ... });

await conn.connect();
```

## Testing

Run the test suite:

```bash
cd web/js/connection
node unified-connection.test.js
```

## Implementation Details

The unified architecture uses the adapter pattern:

- **`Connection`**: Abstract base class defining the interface
- **`LiveConnection`**: Adapter wrapping `live-connection.js` (WebSocket)
- **`ArchiveConnection`**: Adapter wrapping `archive.js` (WASM)
- **`createConnection()`**: Factory function for creating the appropriate adapter

All connection state is synchronized with the global state module (`state.js`), which notifies UI components of changes.
