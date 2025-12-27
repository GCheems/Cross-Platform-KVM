#!/bin/bash
# Verification script for project structure

set -e

echo "Verifying Cross-Platform KVM Project Structure..."
echo ""

# Check for required files
echo "Checking required files..."
files=(
    "Cargo.toml"
    "README.md"
    ".gitignore"
    "src/lib.rs"
    "src/main.rs"
    "src/error.rs"
    "src/device.rs"
    "src/network.rs"
    "src/input.rs"
    "src/clipboard.rs"
    "src/config.rs"
    "src/switch.rs"
    "tests/property_tests.rs"
    "tests/common/mod.rs"
    "docs/SETUP.md"
    "docs/ARCHITECTURE.md"
)

for file in "${files[@]}"; do
    if [ -f "$file" ]; then
        echo "✓ $file"
    else
        echo "✗ $file (missing)"
        exit 1
    fi
done

echo ""
echo "Checking Cargo.toml dependencies..."
if grep -q "proptest" Cargo.toml; then
    echo "✓ proptest configured"
else
    echo "✗ proptest not found"
    exit 1
fi

if grep -q "async-trait" Cargo.toml; then
    echo "✓ async-trait configured"
else
    echo "✗ async-trait not found"
    exit 1
fi

if grep -q "tokio" Cargo.toml; then
    echo "✓ tokio configured"
else
    echo "✗ tokio not found"
    exit 1
fi

echo ""
echo "Checking trait definitions..."
traits=(
    "DeviceDiscovery"
    "ConnectionManager"
    "InputCapture"
    "InputInjection"
    "ClipboardService"
    "ConfigurationManager"
    "SwitchController"
)

for trait in "${traits[@]}"; do
    if grep -r "trait $trait" src/; then
        echo "✓ $trait trait defined"
    else
        echo "✗ $trait trait not found"
        exit 1
    fi
done

echo ""
echo "✅ All structure checks passed!"
echo ""
echo "Next steps:"
echo "1. Install Rust: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
echo "2. Build project: cargo build"
echo "3. Run tests: cargo test"
