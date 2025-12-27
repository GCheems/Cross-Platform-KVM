#!/bin/bash
# Script to generate placeholder icons for the application
# In production, replace this with actual branded icons

set -e

ICONS_DIR="icons"
mkdir -p "$ICONS_DIR"

echo "Generating placeholder icons..."

# Check if ImageMagick is available
if ! command -v convert &> /dev/null; then
    echo "Warning: ImageMagick not found. Please install it to generate icons."
    echo "  macOS: brew install imagemagick"
    echo "  Ubuntu: sudo apt-get install imagemagick"
    echo ""
    echo "Creating placeholder files instead..."
    
    # Create placeholder files
    touch "$ICONS_DIR/32x32.png"
    touch "$ICONS_DIR/128x128.png"
    touch "$ICONS_DIR/128x128@2x.png"
    touch "$ICONS_DIR/icon.png"
    touch "$ICONS_DIR/icon.ico"
    touch "$ICONS_DIR/icon.icns"
    
    echo "Placeholder icon files created. Replace with actual icons before building."
    exit 0
fi

# Create a simple colored square as base icon (placeholder)
# In production, use your actual logo/icon design
convert -size 512x512 xc:#4A90E2 \
    -gravity center \
    -pointsize 200 \
    -fill white \
    -annotate +0+0 "KVM" \
    "$ICONS_DIR/icon_512.png"

# Generate different sizes
convert "$ICONS_DIR/icon_512.png" -resize 32x32 "$ICONS_DIR/32x32.png"
convert "$ICONS_DIR/icon_512.png" -resize 128x128 "$ICONS_DIR/128x128.png"
convert "$ICONS_DIR/icon_512.png" -resize 256x256 "$ICONS_DIR/128x128@2x.png"
convert "$ICONS_DIR/icon_512.png" -resize 128x128 "$ICONS_DIR/icon.png"

# Generate Windows .ico (multiple sizes in one file)
convert "$ICONS_DIR/icon_512.png" \
    \( -clone 0 -resize 16x16 \) \
    \( -clone 0 -resize 32x32 \) \
    \( -clone 0 -resize 48x48 \) \
    \( -clone 0 -resize 64x64 \) \
    \( -clone 0 -resize 128x128 \) \
    \( -clone 0 -resize 256x256 \) \
    -delete 0 "$ICONS_DIR/icon.ico"

# Generate macOS .icns
if [[ "$OSTYPE" == "darwin"* ]]; then
    # Create iconset directory
    ICONSET="$ICONS_DIR/icon.iconset"
    mkdir -p "$ICONSET"
    
    # Generate all required sizes for macOS
    convert "$ICONS_DIR/icon_512.png" -resize 16x16 "$ICONSET/icon_16x16.png"
    convert "$ICONS_DIR/icon_512.png" -resize 32x32 "$ICONSET/icon_16x16@2x.png"
    convert "$ICONS_DIR/icon_512.png" -resize 32x32 "$ICONSET/icon_32x32.png"
    convert "$ICONS_DIR/icon_512.png" -resize 64x64 "$ICONSET/icon_32x32@2x.png"
    convert "$ICONS_DIR/icon_512.png" -resize 128x128 "$ICONSET/icon_128x128.png"
    convert "$ICONS_DIR/icon_512.png" -resize 256x256 "$ICONSET/icon_128x128@2x.png"
    convert "$ICONS_DIR/icon_512.png" -resize 256x256 "$ICONSET/icon_256x256.png"
    convert "$ICONS_DIR/icon_512.png" -resize 512x512 "$ICONSET/icon_256x256@2x.png"
    convert "$ICONS_DIR/icon_512.png" -resize 512x512 "$ICONSET/icon_512x512.png"
    convert "$ICONS_DIR/icon_512.png" -resize 1024x1024 "$ICONSET/icon_512x512@2x.png"
    
    # Convert to icns
    iconutil -c icns "$ICONSET" -o "$ICONS_DIR/icon.icns"
    rm -rf "$ICONSET"
else
    echo "Skipping .icns generation (macOS only)"
    touch "$ICONS_DIR/icon.icns"
fi

echo "Icons generated successfully in $ICONS_DIR/"
echo "Note: These are placeholder icons. Replace with branded icons for production."
