# Build Scripts

This directory contains scripts for building and packaging the Cross-Platform KVM application.

## Available Scripts

### `build.sh`
Universal build script that detects the platform and runs the appropriate build script.

**Usage:**
```bash
./scripts/build.sh [options]
```

**Platforms:** macOS, Windows (via Git Bash)

---

### `build_windows.ps1`
Windows-specific build script for creating MSI installers.

**Usage:**
```powershell
# Development build
.\scripts\build_windows.ps1

# Release build
.\scripts\build_windows.ps1 -Release

# Signed release build
.\scripts\build_windows.ps1 -Release -Sign -CertThumbprint "YOUR_THUMBPRINT"
```

**Requirements:**
- PowerShell 5.1+
- Rust toolchain
- Node.js
- WiX Toolset 3.11+
- Code signing certificate (optional, for signing)

**Output:** `target/release/bundle/msi/*.msi`

---

### `build_macos.sh`
macOS-specific build script for creating DMG installers.

**Usage:**
```bash
# Development build
./scripts/build_macos.sh

# Release build
./scripts/build_macos.sh --release

# Signed release build
./scripts/build_macos.sh --release --sign "Developer ID Application: Name (TEAM_ID)"

# Signed and notarized build
./scripts/build_macos.sh --release \
  --sign "Developer ID Application: Name (TEAM_ID)" \
  --notarize "apple-id@example.com" "TEAM_ID" "app-password"
```

**Requirements:**
- Bash 4.0+
- Rust toolchain
- Node.js
- Xcode Command Line Tools
- ImageMagick (optional, for icon generation)
- Developer ID certificate (optional, for signing)
- Apple Developer account (optional, for notarization)

**Output:** `target/release/bundle/dmg/*.dmg`

---

### `generate_icons.sh`
Generates application icons in all required formats from a base image.

**Usage:**
```bash
./scripts/generate_icons.sh
```

**Requirements:**
- ImageMagick (for actual icon generation)
- macOS (for .icns generation)

**Output:** `icons/` directory with:
- `32x32.png`
- `128x128.png`
- `128x128@2x.png`
- `icon.png`
- `icon.ico` (Windows)
- `icon.icns` (macOS)

**Note:** If ImageMagick is not available, creates placeholder files instead.

---

### `verify_structure.sh`
Verifies the project structure and dependencies.

**Usage:**
```bash
./scripts/verify_structure.sh
```

**Checks:**
- Project directory structure
- Required files exist
- Dependencies are installed

---

## Quick Reference

| Task | Command |
|------|---------|
| Build for current platform | `./scripts/build.sh` |
| Build Windows MSI | `.\scripts\build_windows.ps1 -Release` |
| Build macOS DMG | `./scripts/build_macos.sh --release` |
| Generate icons | `./scripts/generate_icons.sh` |
| Verify project | `./scripts/verify_structure.sh` |

## Build Output Locations

| Platform | Debug Build | Release Build |
|----------|-------------|---------------|
| Windows | `target/debug/bundle/msi/` | `target/release/bundle/msi/` |
| macOS | `target/debug/bundle/dmg/` | `target/release/bundle/dmg/` |

## Environment Variables

### Windows Build

- `WINDOWS_CERTIFICATE_THUMBPRINT` - Certificate thumbprint for signing
- `TIMESTAMP_URL` - Timestamp server URL (default: http://timestamp.digicert.com)

### macOS Build

- `APPLE_SIGNING_IDENTITY` - Code signing identity
- `APPLE_ID` - Apple ID for notarization
- `APPLE_TEAM_ID` - Apple Developer Team ID
- `APPLE_APP_PASSWORD` - App-specific password for notarization

## Troubleshooting

### "Permission denied" when running scripts

**Solution:**
```bash
chmod +x scripts/*.sh
```

### "Execution policy" error on Windows

**Solution:**
```powershell
Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser
```

Or run with bypass:
```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\build_windows.ps1
```

### Build fails with "WiX not found"

**Solution:** Install WiX Toolset from https://wixtoolset.org/releases/

### Build fails with "codesign not found"

**Solution:** Install Xcode Command Line Tools:
```bash
xcode-select --install
```

### Icons not generated

**Solution:** Install ImageMagick:
```bash
# macOS
brew install imagemagick

# Ubuntu/Debian
sudo apt-get install imagemagick
```

Or use placeholder icons (created automatically).

## Additional Documentation

- **Comprehensive Guide:** `docs/PACKAGING.md`
- **Quick Start:** `docs/BUILD_QUICK_START.md`
- **Architecture:** `docs/ARCHITECTURE.md`

## CI/CD

For automated builds, see `.github/workflows/build-installers.yml`

The workflow automatically:
- Builds for both platforms
- Signs installers (when credentials available)
- Creates GitHub releases
- Uploads artifacts

## Contributing

When adding new build scripts:

1. Make scripts executable: `chmod +x script.sh`
2. Add error handling: `set -e` for bash, `$ErrorActionPreference = "Stop"` for PowerShell
3. Add usage documentation in this README
4. Test on clean environment
5. Update CI/CD workflow if needed
