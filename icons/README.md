# Application Icons

This directory contains the application icons for different platforms.

## Icon Files

- `icon.icns` - macOS application icon
- `icon.ico` - Windows application icon
- `32x32.png` - 32x32 PNG icon
- `128x128.png` - 128x128 PNG icon
- `128x128@2x.png` - 128x128@2x PNG icon (Retina)
- `icon.png` - System tray icon

## Generating Icons

To generate icons from a source image, you can use the following tools:

### macOS (.icns)
```bash
# Create iconset directory
mkdir icon.iconset
# Copy and resize images to iconset
# Then convert to icns
iconutil -c icns icon.iconset
```

### Windows (.ico)
Use tools like ImageMagick or online converters to create .ico files from PNG sources.

## Placeholder Icons

The current icons are placeholders. For production, replace these with proper branded icons.
