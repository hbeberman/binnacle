# Embedding the Binnacle Viewer

The binnacle viewer can be embedded in web pages to display project task graphs. This document covers how to build, export, and embed the viewer.

## Overview

The binnacle viewer is a WebAssembly (WASM) application that renders task graphs in the browser. It can load `.bng` archive files containing project data and display them as an interactive graph visualization.

**Use cases:**

- Share project status with stakeholders (no server required)
- Embed task visualizations in documentation sites
- Archive project snapshots for historical reference
- Create standalone reports that work offline

## Quick Start: npm Package

The easiest way to use the viewer is via the npm package:

```bash
npm install @binnacle/viewer
```

```javascript
const { viewerPath } = require('@binnacle/viewer');
// viewerPath is the absolute path to viewer.html
// Copy it to your static assets or serve it directly
```

See the [@binnacle/viewer README](../npm/README.md) for detailed usage examples.

## Building the Viewer

### Prerequisites

- Rust toolchain
- wasm-pack: `cargo install wasm-pack`
- Python 3 (for the embed script)

### Build Commands

```bash
# Build the self-contained viewer.html
just build-viewer

# Build in release mode (smaller, optimized WASM)
just build-viewer-release
```

The output is a single HTML file at `target/viewer/viewer.html` that contains:

- The complete WASM module (base64 encoded)
- All JavaScript glue code
- Full CSS styling
- No external dependencies

### Manual Build Steps

If you need more control over the build:

```bash
# 1. Build the WASM module
wasm-pack build --target web --features wasm --release

# 2. Generate the embedded HTML
./scripts/embed_wasm.sh --skip-build --release
```

## Exporting Project Data

Create a `.bng` archive from your project's binnacle data:

```bash
# Export to a file
bn system store export project.bng

# Export to stdout (for piping)
bn system store export -
```

The `.bng` format is a zstd-compressed tar archive containing:

- `tasks.jsonl` - All entities (tasks, bugs, ideas, milestones, etc.)
- `bugs.jsonl` - Bug entities
- `edges.jsonl` - Entity relationships
- `commits.jsonl` - Commit-to-entity links
- `test-results.jsonl` - Test execution history
- `manifest.json` - Archive metadata

## Using the Viewer

### Standalone Viewer

Open `target/viewer/viewer.html` in a browser. You can:

1. **Drag and drop** a `.bng` file onto the page
2. **Click the upload button** to select a file
3. **Load from URL** by appending `?url=https://example.com/project.bng`

### Embedding in a Web Page

#### Method 1: iframe

The simplest approach - embed the viewer HTML in an iframe:

```html
<iframe 
  src="viewer.html?url=./project.bng" 
  width="100%" 
  height="600px"
  frameborder="0">
</iframe>
```

#### Method 2: Direct Embedding

For full control, include the WASM module directly in your page:

```html
<script type="module">
import init, { BinnacleViewer } from './binnacle_wasm.js';

async function main() {
    // Initialize the WASM module
    await init();
    
    // Create viewer instance
    const viewer = new BinnacleViewer();
    
    // Load from URL
    await viewer.loadFromUrl('https://example.com/project.bng');
    
    // Or load from bytes
    // const response = await fetch('./project.bng');
    // const bytes = new Uint8Array(await response.arrayBuffer());
    // viewer.loadFromBytes(bytes);
    
    // Get the canvas element
    const canvas = document.getElementById('graph-canvas');
    
    // Render the graph
    viewer.render(canvas);
}

main();
</script>

<canvas id="graph-canvas" width="1200" height="800"></canvas>
```

## URL Parameters

The viewer supports these URL parameters:

| Parameter | Description | Example |
|-----------|-------------|---------|
| `url` | URL of `.bng` file to load | `?url=./project.bng` |

## Hosting Considerations

### Static Hosting

The self-contained `viewer.html` can be hosted on any static file server:

- GitHub Pages
- Netlify
- S3/CloudFront
- Any web server

### CORS

If loading `.bng` files from a different origin, ensure the server sets appropriate CORS headers:

```
Access-Control-Allow-Origin: *
```

### File Size

Typical sizes for the self-contained viewer:

- Debug build: ~2-3 MB
- Release build: ~500 KB - 1 MB

The release build uses `wasm-opt` for optimization.

## Workflow Example

Here's a complete workflow for sharing project status:

```bash
# 1. Export your project data
bn system store export project.bng

# 2. Build the viewer (if not already done)
just build-viewer-release

# 3. Copy both files to your hosting location
cp target/viewer/viewer.html /var/www/project/
cp project.bng /var/www/project/

# 4. Access via browser
# https://example.com/project/viewer.html?url=./project.bng
```

## Archive Format Details

### manifest.json

```json
{
  "version": 1,
  "format": "binnacle-store-v1",
  "exported_at": "2026-01-25T12:00:00Z",
  "source_repo": "/path/to/repo",
  "binnacle_version": "0.1.0"
}
```

### Entity Types

The viewer renders these entity types:

| Type | ID Prefix | Visual Style |
|------|-----------|--------------|
| Task | `bn-` | Blue nodes |
| Bug | `bn-` | Red nodes |
| Idea | `bn-` | Purple nodes |
| Milestone | `bn-` | Large diamond |
| Queue | `bnq-` | Teal hexagon |
| Doc | `bn-` | Yellow/orange based on type |
| Agent | `bn-` | Cyan circle |

### Edge Types

| Type | Visual Style |
|------|--------------|
| `depends_on` | Red arrow |
| `child_of` | Purple dashed line |
| `queued` | Teal line |
| `fixes` | Green line |

## Troubleshooting

### Viewer shows blank page

- Check browser console for errors
- Ensure the `.bng` file is accessible (CORS)
- Verify the file is a valid binnacle archive

### Graph doesn't render

- Check that the archive contains entities (not empty)
- Look for JavaScript errors in console

### WASM fails to load

- Ensure your browser supports WebAssembly
- Try a recent version of Chrome, Firefox, or Safari
- Check for Content Security Policy blocking WASM execution

## See Also

- [Getting Started](./getting-started.md) - Basic binnacle usage
- [PRD](../PRD.md) - Design philosophy and full feature list
