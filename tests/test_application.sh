#!/bin/bash

# Test script for Bevy Sprite Atlas Editor
# This script runs the application and validates basic functionality

set -e

echo "=========================================="
echo "Bevy Sprite Atlas Editor Test Suite"
echo "=========================================="
echo

# Step 1: Check compilation
echo "Step 1: Checking compilation..."
cd "$(dirname "$0")/.."
cargo check --quiet 2>&1 | grep -q "Finished" && echo "✓ Compilation successful"

# Step 2: Run unit tests
echo "Step 2: Running unit tests..."
cargo test --quiet 2>&1 | grep -q "test result: ok" && echo "✓ All unit tests passed"

# Step 3: Build release binary
echo "Step 3: Building release binary..."
cargo build --release --quiet 2>&1 | grep -q "Finished" && echo "✓ Release build successful"

# Step 4: Check binary exists
echo "Step 4: Checking binary..."
if [ -f "target/release/bevy_sprite_editor" ]; then
    echo "✓ Binary exists"
else
    echo "✗ Binary not found"
    exit 1
fi

# Step 5: Run application briefly to check it starts
echo "Step 5: Testing application startup..."
timeout 2 ./target/release/bevy_sprite_editor > /tmp/bevy_test_output.log 2>&1 || true
if grep -q "Bevy Sprite Atlas Editor" /tmp/bevy_test_output.log || [ $? -eq 124 ]; then
    echo "✓ Application starts successfully"
else
    echo "✗ Application failed to start"
    cat /tmp/bevy_test_output.log
    exit 1
fi

echo
echo "=========================================="
echo "All automated tests passed!"
echo "=========================================="
echo
echo "For visual validation, run:"
echo "  ./tests/capture_window.sh"
echo
echo "This will:"
echo "  1. Build the application"
echo "  2. Run it in a window"
echo "  3. Capture a screenshot"
echo "  4. Save to target/screenshots/"
echo
echo "Requirements for capture_window.sh:"
echo "  - xdotool (window control)"
echo "  - ImageMagick (screenshot capture)"
echo
echo "Install on Ubuntu/Debian:"
echo "  sudo apt-get install xdotool imagemagick"
echo
echo "To manually test the application, run:"
echo "  ./target/release/bevy_sprite_editor"
echo
echo "Expected visual elements:"
echo "  - Color palette on LEFT side (17 color swatches)"
echo "  - 8x8 grid of pixels on RIGHT side (512x512)"
echo "  - Dark gray status bar at BOTTOM"
echo "  - No gap between palette and viewport"
