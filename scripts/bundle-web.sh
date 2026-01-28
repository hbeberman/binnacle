#!/usr/bin/env bash
# Bundle web assets using esbuild and compress with zstd

set -e

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}Bundling web assets...${NC}"

# Create output directory
BUNDLE_DIR="target/web-bundle"
rm -rf "$BUNDLE_DIR"
mkdir -p "$BUNDLE_DIR"

# Find all JS files in web/js/ directory (except state.js)
echo "Bundling JavaScript..."
find web/js -name "*.js" ! -name "*.test.js" ! -name "state.js" -type f | while read -r jsfile; do
    # Get relative path from web/js
    relpath="${jsfile#web/js/}"
    outfile="$BUNDLE_DIR/js/$relpath"
    mkdir -p "$(dirname "$outfile")"
    
    # Use esbuild to bundle and minify (includes npm dependencies like marked, highlight.js)
    # Keep state.js external to avoid duplicate module instances
    npx esbuild "$jsfile" --bundle --minify --format=esm --external:../state.js --outfile="$outfile"
done

# Copy state.js as-is (don't bundle it to avoid circular dependency issues)
echo "Copying state.js..."
mkdir -p "$BUNDLE_DIR/js"
cp web/js/state.js "$BUNDLE_DIR/js/state.js"

# Bundle all CSS files into a single main.css
echo "Bundling CSS..."
mkdir -p "$BUNDLE_DIR/css"

# Concatenate all CSS files: main.css, theme, and components
cat web/css/main.css > "$BUNDLE_DIR/css/main.css"
cat web/css/themes/dark.css >> "$BUNDLE_DIR/css/main.css"
find web/css/components -name "*.css" -type f | while read -r cssfile; do
    cat "$cssfile" >> "$BUNDLE_DIR/css/main.css"
done

# Minify the bundled CSS
npx esbuild "$BUNDLE_DIR/css/main.css" --minify --outfile="$BUNDLE_DIR/css/main.css.tmp"
mv "$BUNDLE_DIR/css/main.css.tmp" "$BUNDLE_DIR/css/main.css"

# Process index.html to use only bundled main.css
echo "Processing index.html..."
# Remove all individual CSS component links, keep only /css/main.css
sed '/<link rel="stylesheet" href="\/css\/themes\/dark.css">/d' web/index.html | \
sed '/<link rel="stylesheet" href="\/css\/components\//d' > "$BUNDLE_DIR/index.html"

# Copy assets directory if it has files
if [ -n "$(ls -A web/assets 2>/dev/null | grep -v '.gitkeep')" ]; then
    echo "Copying assets..."
    mkdir -p "$BUNDLE_DIR/assets"
    cp -r web/assets/* "$BUNDLE_DIR/assets/" 2>/dev/null || true
fi

# Create compressed archive with zstd
echo "Compressing bundle with zstd..."
cd target
tar cf - web-bundle | zstd -19 -f -o web-bundle.tar.zst
cd ..

# Show bundle size
BUNDLE_SIZE=$(du -sh "$BUNDLE_DIR" | cut -f1)
ARCHIVE_SIZE=$(du -sh target/web-bundle.tar.zst | cut -f1)

echo -e "${GREEN}✓ Web assets bundled successfully${NC}"
echo "  Bundle directory: $BUNDLE_DIR ($BUNDLE_SIZE)"
echo "  Compressed archive: target/web-bundle.tar.zst ($ARCHIVE_SIZE)"
