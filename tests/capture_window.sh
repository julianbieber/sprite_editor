#!/bin/bash

# Screenshot capture script for Bevy Sprite Atlas Editor
# Uses xdotool and scrot/import to capture the application window
#
# Requirements:
#   - xdotool (for window control)
#   - scrot or ImageMagick (for screenshot capture)
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
if ! command -v xdotool &> /dev/null; then
    echo "ERROR: 'xdotool' not found. Please install:"
    echo "  sudo apt-get install xdotool"
    exit 1
fi

# Prefer scrot over import
SCREENSHOT_CMD=""
if command -v scrot &> /dev/null; then
    SCREENSHOT_CMD="scrot"
elif command -v import &> /dev/null; then
    SCREENSHOT_CMD="import"
else
    echo "ERROR: Neither 'scrot' nor 'import' found. Please install:"
    echo "  scrot: sudo apt-get install scrot"
    echo "  ImageMagick: sudo apt-get install imagemagick"
    exit 1
fi

echo "✓ Requirements met (xdotool, $SCREENSHOT_CMD)"
echo

# Build the application in release mode
echo "Building application..."
cd "$PROJECT_DIR"
cargo build --release --quiet 2>&1 | grep -q "Finished" || true

echo "✓ Build complete"
echo

# Run the application in the background
BINARY="$PROJECT_DIR/target/release/bevy_sprite_editor"
$BINARY > /tmp/bevy_window.log 2>&1 &
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
    cat /tmp/bevy_window.log
    kill $APP_PID 2>/dev/null || true
    exit 1
fi

# Move window to center of screen
xdotool windowmove $WINDOW_ID 0 0
sleep 0.5

# Raise and focus the window
xdotool windowraise $WINDOW_ID
xdotool windowactivate $WINDOW_ID
xdotool windowfocus $WINDOW_ID
sleep 2

# Get final window geometry
echo "Window geometry:"
xdotool getwindowgeometry $WINDOW_ID

# Capture the window screenshot
echo "Capturing screenshot..."
if [ "$SCREENSHOT_CMD" = "scrot" ]; then
    scrot --border 0 -u "$OUTPUT_FILE"
else
    import -window $WINDOW_ID "$OUTPUT_FILE"
fi

if [ -f "$OUTPUT_FILE" ]; then
    echo "✓ Screenshot saved to: $OUTPUT_FILE"
    echo
    echo "Image info:"
    file "$OUTPUT_FILE"
    echo
    if command -v identify &> /dev/null; then
        identify "$OUTPUT_FILE" 2>/dev/null | head -1
    fi
    echo
    echo "File size: $(du -h "$OUTPUT_FILE" | cut -f1)"
    echo
echo "You can view the screenshot with:"
    echo "  eog $OUTPUT_FILE"
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
