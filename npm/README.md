# @binnacle/viewer

Self-contained web viewer for [binnacle](https://github.com/hbeberman/binnacle) task graphs.

## Overview

This package provides a standalone HTML file that displays `.bng` archives (binnacle project exports) in a web browser with an interactive graph visualization. The viewer is completely self-contained - all WebAssembly, JavaScript, and CSS are embedded inline.

## Installation

```bash
npm install @binnacle/viewer
# or
yarn add @binnacle/viewer
```

## Usage

### Get the Viewer Path

```javascript
const { viewerPath } = require('@binnacle/viewer');
console.log('Viewer at:', viewerPath);
// Copy to your static assets, serve with your web server, etc.
```

### Copy to Static Assets

```javascript
const { viewerPath } = require('@binnacle/viewer');
const fs = require('fs');
const path = require('path');

// Copy viewer to your public directory
fs.copyFileSync(viewerPath, path.join(__dirname, 'public', 'viewer.html'));
```

### Serve with Express

```javascript
const express = require('express');
const { viewerPath } = require('@binnacle/viewer');
const path = require('path');

const app = express();

// Serve the viewer at /viewer
app.get('/viewer', (req, res) => {
  res.sendFile(viewerPath);
});

// Serve .bng files from a data directory
app.use('/data', express.static(path.join(__dirname, 'bng-files')));

app.listen(3000);
// Access viewer at: http://localhost:3000/viewer?url=/data/project.bng
```

### Embed in HTML (iframe)

```html
<iframe 
  src="/viewer.html?url=./project.bng" 
  width="100%" 
  height="600px"
  frameborder="0">
</iframe>
```

## Creating .bng Archives

Export your binnacle project data using the CLI:

```bash
# Export to a file
bn system store export project.bng

# Export to stdout (for piping)
bn system store export -
```

## URL Parameters

The viewer supports these URL parameters:

| Parameter | Description | Example |
|-----------|-------------|---------|
| `url` | URL of `.bng` file to load | `?url=./project.bng` |

## Features

- **Self-contained**: Single HTML file with embedded WASM, JS, and CSS
- **Interactive graph**: Pan, zoom, and explore task relationships
- **Drag & drop**: Load `.bng` files by dragging onto the page
- **URL loading**: Load archives from URLs via query parameter
- **Offline capable**: Works without network after initial load

## Entity Types Rendered

| Type | ID Prefix | Visual Style |
|------|-----------|--------------|
| Task | `bn-` | Blue nodes |
| Bug | `bn-` | Red nodes |
| Idea | `bn-` | Purple nodes |
| Milestone | `bn-` | Large diamond |
| Queue | `bnq-` | Teal hexagon |

## Requirements

- Modern browser with WebAssembly support (Chrome, Firefox, Safari, Edge)
- Node.js 16+ (for the npm package)

## License

MIT - See [LICENSE](https://github.com/hbeberman/binnacle/blob/main/LICENSE)

## Related

- [binnacle](https://github.com/hbeberman/binnacle) - The CLI tool and full project
- [Embedding Guide](https://github.com/hbeberman/binnacle/blob/main/docs/embedding-viewer.md) - Detailed embedding documentation
