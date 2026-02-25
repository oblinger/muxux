#!/bin/bash
# Launch MuxUX in development mode.
# Starts the vite dev server, then runs the Tauri binary.
# Ctrl+C stops both.

cd "$(dirname "$0")"

# Kill any leftover processes
pkill -f "vite.*1420" 2>/dev/null
pkill -f "target/debug/muxux" 2>/dev/null

# Start vite dev server in background
npx vite --port 1420 frontend/ &
VITE_PID=$!

# Wait for dev server
for i in $(seq 1 10); do
    curl -s http://localhost:1420 >/dev/null 2>&1 && break
    sleep 0.5
done

# Build and run
cargo build -p muxux 2>&1 && ./target/debug/muxux

# Cleanup on exit
kill $VITE_PID 2>/dev/null
