//! Layout management — target resolution, snapshot reconstruction, and capture.
//!
//! The `targeting` module resolves agent names and P-notation strings to
//! concrete tmux pane identifiers. The `snapshot` module reconstructs a
//! `LayoutNode` tree from raw pane geometry data. The `capture` module
//! wires together parsing, reconstruction, and diffing into an end-to-end
//! pipeline. The `timer` module schedules periodic captures.

pub mod capture;
pub mod snapshot;
pub mod targeting;
pub mod timer;
