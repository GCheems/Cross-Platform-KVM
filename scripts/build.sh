#!/bin/bash
# Universal Build Script for Cross-Platform KVM
# Detects the platform and runs the appropriate build script

set -e

echo "========================================"
echo "Cross-Platform KVM - Universal Build"
echo "========================================"
echo ""

# Detect platform
if [[ "$OSTYPE" == "darwin"* ]]; then
    PLATFORM="macOS"
    BUILD_SCRIPT="scripts/build_macos.sh"
elif [[ "$OSTYPE" == "msys" ]] || [[ "$OSTYPE" == "cygwin" ]] || [[ "$OSTYPE" == "win32" ]]; then
    PLATFORM="Windows"
    BUILD_SCRIPT="scripts/build_windows.ps1"
else
    echo "Error: Unsupported platform: $OSTYPE"
    echo "This script supports macOS and Windows only."
    exit 1
fi

echo "Detected platform: $PLATFORM"
echo ""

# Run platform-specific build script
if [ "$PLATFORM" = "macOS" ]; then
    if [ ! -f "$BUILD_SCRIPT" ]; then
        echo "Error: Build script not found: $BUILD_SCRIPT"
        exit 1
    fi
    
    echo "Running macOS build script..."
    bash "$BUILD_SCRIPT" "$@"
elif [ "$PLATFORM" = "Windows" ]; then
    if [ ! -f "$BUILD_SCRIPT" ]; then
        echo "Error: Build script not found: $BUILD_SCRIPT"
        exit 1
    fi
    
    echo "Running Windows build script..."
    echo "Please run: powershell -ExecutionPolicy Bypass -File $BUILD_SCRIPT"
    echo ""
    echo "Or on Windows, use PowerShell directly:"
    echo "  .\\scripts\\build_windows.ps1"
fi

echo ""
echo "Build script execution complete."
