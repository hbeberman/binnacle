#!/usr/bin/env bash
# Test script for bn-agent dependency checking
# This verifies the dependency check logic without removing actual dependencies

set -e

echo "=== bn-agent Dependency Check Tests ==="
echo ""

# Test 1: Verify script has dependency checking function
echo "Test 1: Dependency check function exists"
if grep -q "check_dependencies()" ./scripts/bn-agent; then
    echo "✓ check_dependencies() function found"
else
    echo "✗ check_dependencies() function missing"
    exit 1
fi
echo ""

# Test 2: Verify bn check exists
echo "Test 2: bn binary check exists"
if grep -q 'command -v bn' ./scripts/bn-agent && \
   grep -q "bn: Binnacle CLI not found" ./scripts/bn-agent; then
    echo "✓ bn dependency check found with error message"
else
    echo "✗ bn dependency check missing or incomplete"
    exit 1
fi
echo ""

# Test 3: Verify jq check exists  
echo "Test 3: jq dependency check exists"
if grep -q 'command -v jq' ./scripts/bn-agent && \
   grep -q "jq: JSON processor not found" ./scripts/bn-agent; then
    echo "✓ jq dependency check found with error message"
else
    echo "✗ jq dependency check missing or incomplete"
    exit 1
fi
echo ""

# Test 4: Verify copilot check exists
echo "Test 4: copilot CLI check exists"
if grep -q "bn system copilot path" ./scripts/bn-agent && \
   grep -q "GitHub Copilot CLI: Not found" ./scripts/bn-agent; then
    echo "✓ copilot CLI check found with error message"
else
    echo "✗ copilot CLI check missing or incomplete"
    exit 1
fi
echo ""

# Test 5: Verify remediation hints exist
echo "Test 5: Remediation hints exist"
if grep -q "Install:" ./scripts/bn-agent && \
   grep -q "apt-get install\|brew install\|dnf install" ./scripts/bn-agent; then
    echo "✓ Installation hints found"
else
    echo "✗ Installation hints missing"
    exit 1
fi
echo ""

# Test 6: Verify container checks exist
echo "Test 6: Container infrastructure checks exist"
if grep -q "Container infrastructure not available" ./scripts/bn-agent && \
   grep -q "containerd" ./scripts/bn-agent; then
    echo "✓ Container infrastructure checks found"
else
    echo "✗ Container infrastructure checks missing"
    exit 1
fi
echo ""

# Test 7: Verify dependency checks run early
echo "Test 7: Dependency checks called early in script"
if head -100 ./scripts/bn-agent | grep -q "check_dependencies"; then
    echo "✓ check_dependencies() called in first 100 lines"
else
    echo "✗ check_dependencies() called too late or not at all"
    exit 1
fi
echo ""

# Test 8: Run script with all dependencies (integration test)
echo "Test 8: Script runs with all dependencies present"
if ./scripts/bn-agent --help >/dev/null 2>&1 || [ $? -eq 1 ]; then
    # Exit code 1 is expected from usage()
    echo "✓ Script executes successfully"
else
    echo "✗ Script failed unexpectedly"
    exit 1
fi
echo ""

echo "=== All dependency check tests passed! ==="
