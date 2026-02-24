# muxux

MuxUX — Structure App. Tauri overlay for tmux session management, layout control, and agent placement.

## Structure

- core/ — domain logic (layout, session, infrastructure, structure commands)
- tauri/ — Tauri app (Rust backend + TypeScript frontend)
- cli/ — command-line client (mux binary)
- frontend/ — TypeScript/HTML for webview

## Build

cargo test
cargo build -p mux-cli
npm run build (in frontend/)

## Dependencies

- cmx-utils (socket protocol, logging)
