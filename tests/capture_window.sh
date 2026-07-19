#!/bin/bash

# Screenshot capture script for Bevy Sprite Atlas Editor
# Uses xdotool and ImageMagick (import) to capture the application window
#
# Requirements:
#   - xdotool (for window control)
#   - ImageMagick (for screenshot capture)
#   - X11 display server
#
# Usage: ./tests/capture_window.sh [output_file.png]

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

# Default output filename
OUTPUT_FILE="${PROJECT_DIR}/target/screenshots/editor_$(date +%Y%m%d_%H%M%S).png"

# Use custom output if provided
if [ $# -ge 1 ]; then
    OUTPUT_FILE="$1"
fi

# Create screenshots directory
mkdir -p "$(dirname "$OUTPUT_FILE")"

echo "=========================================="
echo "Capturing Bevy Sprite Atlas Editor Window"
echo "=========================================="
echo "Output: $OUTPUT_FILE"
echo

# Check requirements
for cmd in xdotool import; do
    if ! command -v $cmd &> /dev/null; then
        echo "ERROR: '$cmd' not found. Please install:"
        echo "  xdotool: sudo apt-get install xdotool"
        echo "  ImageMagick: sudo apt-get install imagemagick"
        exit 1
    fi
done

echo "✓ Requirements met (xdotool, import)"
echo

# Build the application in release mode
echo "Building application..."
cd "$PROJECT_DIR"
cargo build --release --quiet 2>&1 | grep -q "Finished" || true

echo "✓ Build complete"
echo

# Run the application in the background
BINARY="$PROJECT_DIR/target/release/bevy_sprite_editor"
$BINARY &
APP_PID=$!

# Wait for the window to appear (up to 10 seconds)
echo "Waiting for window to appear..."
for i in {1..10}; do
    sleep 1
    WINDOW_ID=$(xdotool search --name "Bevy Sprite Atlas Editor" 2>/dev/null | head -1)
    if [ -n "$WINDOW_ID" ]; then
        echo "✓ Window found (ID: $WINDOW_ID)"
        break
    fi
    echo "  Attempt $i/10..."
done

if [ -z "$WINDOW_ID" ]; then
    echo "✗ Window did not appear after 10 seconds"
    kill $APP_PID 2>/dev/null || true
    exit 1
fi

# Activate and focus the window
xdotool windowactivate $WINDOW_ID
sleep 0.5

# Capture the window screenshot
echo "Capturing screenshot..."
import -window $WINDOW_ID "$OUTPUT_FILE"

if [ -f "$OUTPUT_FILE" ]; then
    echo "✓ Screenshot saved to: $OUTPUT_FILE"
    echo
    echo "Image info:"
    identify "$OUTPUT_FILE" 2>/dev/null | head -1 || echo "  (identify not available)"
    echo
    echo "File size: $(du -h "$OUTPUT_FILE" | cut -f1)"
else
    echo "✗ Failed to capture screenshot"
    exit 1
fi

# Close the application
sleep 0.5
kill $APP_PID 2>/dev/null || true

echo
echo "=========================================="
echo "Screenshot Test Complete"
echo "=========================================="
echo
echo "To view the screenshot:"
echo "  xdg-open $OUTPUT_FILE"
echo
echo "Or manually inspect:"
echo "  eog $OUTPUT_FILE"
echo "  firefox $OUTPUT_FILE"
