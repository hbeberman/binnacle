/**
 * @binnacle/viewer - Self-contained web viewer for binnacle task graphs
 * 
 * This package provides a standalone HTML file that can display .bng archives
 * (binnacle project exports) in a web browser with an interactive graph visualization.
 * 
 * @example
 * // Get the path to viewer.html for serving
 * const { viewerPath } = require('@binnacle/viewer');
 * console.log('Viewer at:', viewerPath);
 * 
 * // Copy to your static assets directory
 * const fs = require('fs');
 * fs.copyFileSync(viewerPath, './public/viewer.html');
 */

const path = require('path');

/**
 * Absolute path to the self-contained viewer.html file.
 * This file contains all WASM, JavaScript, and CSS embedded inline.
 * @type {string}
 */
const viewerPath = path.join(__dirname, 'viewer.html');

/**
 * Package version (matches binnacle version)
 * @type {string}
 */
const version = require('./package.json').version;

module.exports = {
  viewerPath,
  version,
};
