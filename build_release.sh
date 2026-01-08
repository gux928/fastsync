#!/bin/bash
set -e

# Configuration
LINUX_TARGET="x86_64-unknown-linux-gnu"
WINDOWS_TARGET="x86_64-pc-windows-gnu"
OUTPUT_DIR="dist"

# Extract Version from Cargo.toml
VERSION=$(grep "^version" Cargo.toml | head -n 1 | cut -d '"' -f 2)

# Print header
echo "=========================================="
echo "   fastsync Cross-Platform Build Script   "
echo "   Version: $VERSION                      "
echo "=========================================="

# 1. Check for MinGW-w64
if ! command -v x86_64-w64-mingw32-gcc &> /dev/null; then
    echo "❌ Error: MinGW-w64 toolchain not found."
    echo "👉 Install with: sudo apt install mingw-w64"
    exit 1
fi

# 2. Add Rust targets
echo "🛠️  Checking Rust targets..."
rustup target add $LINUX_TARGET
rustup target add $WINDOWS_TARGET

# 3. Prepare output directory
mkdir -p $OUTPUT_DIR
echo "📂 Output directory: $OUTPUT_DIR"

# 4. Build Linux Version
echo "------------------------------------------"
echo "🐧 Building Linux version..."
echo "------------------------------------------"
cargo build --release --target $LINUX_TARGET
cp "target/$LINUX_TARGET/release/fastsync" "$OUTPUT_DIR/fastsync-linux-amd64"
echo "✅ Linux build success"

# 5. Build Linux Debian Package
echo "------------------------------------------"
echo "📦 Packaging Linux .deb..."
echo "------------------------------------------"
if command -v dpkg-deb &> /dev/null; then
    DEB_DIR="packaging/linux"
    rm -rf "$DEB_DIR/usr/bin"
    mkdir -p "$DEB_DIR/usr/bin"
    sed -i "s/^Version: .*/Version: ${VERSION}/" "$DEB_DIR/DEBIAN/control"
    cp "target/$LINUX_TARGET/release/fastsync" "$DEB_DIR/usr/bin/"
    chmod 755 "$DEB_DIR/usr/bin/fastsync"
    dpkg-deb --build "$DEB_DIR" "$OUTPUT_DIR/fastsync_${VERSION}_amd64.deb"
    echo "✅ Debian package created"
else
    echo "⚠️  Warning: 'dpkg-deb' not found. Skipping .deb packaging."
fi

# 6. Build Windows Version
echo "------------------------------------------"
echo "🪟 Building Windows version..."
echo "------------------------------------------"
export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc
cargo build --release --target $WINDOWS_TARGET
cp "target/$WINDOWS_TARGET/release/fastsync.exe" "$OUTPUT_DIR/fastsync-windows-amd64.exe"
echo "✅ Windows build success"

# 7. Package Windows Installer (NSIS)
echo "------------------------------------------"
echo "📦 Packaging Windows Installer (NSIS)..."
echo "------------------------------------------"
if command -v makensis &> /dev/null; then
    makensis -DVERSION=$VERSION packaging/windows/installer.nsi > /dev/null
    if [ -f "dist/fastsync-setup.exe" ]; then
        mv "dist/fastsync-setup.exe" "dist/fastsync-${VERSION}-setup.exe"
        echo "✅ Installer created: dist/fastsync-${VERSION}-setup.exe"
    else
        echo "❌ Error: Installer creation failed."
        exit 1
    fi
else
    echo "⚠️  Warning: 'makensis' not found. Skipping Windows installer packaging."
fi

echo "=========================================="
echo "🎉 Local build completed successfully!"
ls -lh $OUTPUT_DIR/