# MuxUX — dev recipes

default:
    @just --list

# Build all crates (release)
build: check
    cargo build --release

# Build debug
build-debug: check
    cargo build

# Build and run the Tauri app (dev mode)
dev:
    cd tauri && cargo tauri dev

# Run all tests
test:
    cargo test

# Run tests for a specific crate
test-core:
    cargo test -p muxux-core

test-tauri:
    cargo test -p muxux

test-cli:
    cargo test -p mux-cli

# Install mux CLI binary to ~/bin
install: build
    @mkdir -p ~/bin
    @ln -sf "$(pwd)/target/release/mux" ~/bin/mux
    @echo "Installed: ~/bin/mux → target/release/mux"

# Uninstall from ~/bin
uninstall:
    @rm -f ~/bin/mux
    @echo "Removed ~/bin/mux"

# Dev environment check
check:
    @command -v cargo >/dev/null || { echo "ERROR: cargo not found"; exit 1; }
    @command -v rustc >/dev/null || { echo "ERROR: rustc not found"; exit 1; }
    @command -v npm >/dev/null || { echo "ERROR: npm not found"; exit 1; }

# Build frontend
frontend:
    cd frontend && npm run build

# Create /Applications/MuxUX.app dev bundle (symlinks to release binary)
build_dev_env: build frontend
    #!/usr/bin/env bash
    set -e
    APP="/Applications/MuxUX.app"
    REPO="$(pwd)"
    BINARY="$REPO/target/release/muxux"
    ICON="$REPO/tauri/icons/icon.icns"

    echo "Creating $APP (symlink-based dev bundle)..."
    mkdir -p "$APP/Contents/MacOS"
    mkdir -p "$APP/Contents/Resources"

    # Symlink binary — rebuilds are immediately picked up
    ln -sf "$BINARY" "$APP/Contents/MacOS/MuxUX"

    # Copy icon
    cp -f "$ICON" "$APP/Contents/Resources/AppIcon.icns" 2>/dev/null || true

    # Generate Info.plist
    cat > "$APP/Contents/Info.plist" << 'PLIST'
    <?xml version="1.0" encoding="UTF-8"?>
    <!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
    <plist version="1.0">
    <dict>
        <key>CFBundleName</key>
        <string>MuxUX</string>
        <key>CFBundleDisplayName</key>
        <string>MuxUX</string>
        <key>CFBundleIdentifier</key>
        <string>com.muxux.app</string>
        <key>CFBundleExecutable</key>
        <string>MuxUX</string>
        <key>CFBundleIconFile</key>
        <string>AppIcon</string>
        <key>CFBundlePackageType</key>
        <string>APPL</string>
        <key>CFBundleVersion</key>
        <string>0.1.0</string>
        <key>CFBundleShortVersionString</key>
        <string>0.1.0</string>
        <key>LSUIElement</key>
        <true/>
        <key>NSHighResolutionCapable</key>
        <true/>
    </dict>
    </plist>
    PLIST

    # Marker file
    echo "DO NOT COPY BINARIES — this is a symlink-based dev bundle." > "$APP/Contents/DO_NOT_COPY_BINARIES.md"

    # Register with Launch Services
    /System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f "$APP" 2>/dev/null || true

    echo "Done: $APP → $BINARY"
    echo "Run with: open /Applications/MuxUX.app"

# Clean build artifacts
clean:
    cargo clean

# Quick pre-commit gate
test-commit:
    cargo test --lib -p muxux-core -- --quiet
