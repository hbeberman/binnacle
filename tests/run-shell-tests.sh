#!/usr/bin/env bash
# Run all shell-based tests for binnacle

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "========================================="
echo "Running Shell Tests for Binnacle"
echo "========================================="
echo ""

# Track failures
FAILED=0

# Run bn-agent dependency tests
echo "Running bn-agent dependency tests..."
if bash "$SCRIPT_DIR/bn-agent-deps-test.sh"; then
    echo "✓ Dependency tests passed"
else
    echo "✗ Dependency tests failed"
    FAILED=$((FAILED + 1))
fi
echo ""

# Run bn-agent comprehensive tests
echo "Running bn-agent comprehensive tests..."
if bash "$SCRIPT_DIR/bn-agent-comprehensive-test.sh"; then
    echo "✓ Comprehensive tests passed"
else
    echo "✗ Comprehensive tests failed"
    FAILED=$((FAILED + 1))
fi
echo ""

# Summary
echo "========================================="
if [ $FAILED -eq 0 ]; then
    echo "✓ All shell tests passed!"
    exit 0
else
    echo "✗ $FAILED test suite(s) failed"
    exit 1
fi
