//! MuxUX Tauri application library.
//!
//! This crate provides the Tauri backend for the MuxUX GUI (Structure App).
//! It bridges the frontend JavaScript (webview) to the core `Sys` runtime
//! via IPC commands.
//!
//! # Architecture
//!
//! 1. **AppState** (this module) -- wraps `Sys` in a `Mutex` for thread-safe
//!    access from Tauri command handlers.
//!
//! 2. **IPC handlers** (`ipc` module) -- thin `#[tauri::command]` functions
//!    that pull `AppState` from Tauri's managed state and delegate to it.
//!
//! 3. **`run()`** -- assembles the Tauri application, registers all IPC
//!    handlers, and starts the event loop.

pub mod ipc;

use muxux_core::command::Command;
use muxux_core::sys::Sys;
use cmx_utils::response::{Action, Response};
use std::sync::Mutex;
use tauri::Manager;


/// Overlay window state, separate from core AppState.
///
/// Tracks the overlay visibility and the target tmux pane ID.
pub struct OverlayState {
    pub visible: Mutex<bool>,
    pub target_pane: Mutex<Option<String>>,
}


impl OverlayState {
    pub fn new() -> Self {
        OverlayState {
            visible: Mutex::new(false),
            target_pane: Mutex::new(None),
        }
    }

    pub fn show(&self, pane_id: String) {
        *self.target_pane.lock().unwrap() = Some(pane_id);
        *self.visible.lock().unwrap() = true;
    }

    pub fn hide(&self) {
        *self.visible.lock().unwrap() = false;
    }

    pub fn get_target_pane(&self) -> Option<String> {
        self.target_pane.lock().unwrap().clone()
    }

    pub fn is_visible(&self) -> bool {
        *self.visible.lock().unwrap()
    }
}


/// CLI arguments for overlay mode.
#[derive(Debug, Clone)]
pub struct OverlayArgs {
    pub overlay: bool,
    pub x: i32,
    pub y: i32,
    pub pane: String,
}


impl OverlayArgs {
    /// Parse overlay args from the process command line.
    /// Returns None if `--overlay` is not present.
    pub fn from_env() -> Option<OverlayArgs> {
        let args: Vec<String> = std::env::args().collect();
        if !args.iter().any(|a| a == "--overlay") {
            return None;
        }

        let x = Self::parse_flag(&args, "--x").unwrap_or(0);
        let y = Self::parse_flag(&args, "--y").unwrap_or(0);
        let pane = Self::parse_string_flag(&args, "--pane")
            .unwrap_or_default();

        Some(OverlayArgs {
            overlay: true,
            x,
            y,
            pane,
        })
    }

    fn parse_flag(args: &[String], flag: &str) -> Option<i32> {
        args.iter()
            .position(|a| a == flag)
            .and_then(|i| args.get(i + 1))
            .and_then(|v| v.parse().ok())
    }

    fn parse_string_flag(args: &[String], flag: &str) -> Option<String> {
        args.iter()
            .position(|a| a == flag)
            .and_then(|i| args.get(i + 1))
            .map(|v| v.clone())
    }
}


/// Application state shared across Tauri commands.
///
/// Wraps the core `Sys` runtime in a `Mutex` so that IPC command handlers
/// can safely access it from arbitrary threads.
pub struct AppState {
    sys: Mutex<Sys>,
}


impl AppState {
    /// Create a new AppState with the given project root.
    pub fn new(project_root: String) -> AppState {
        let sys = Sys::new(project_root);
        AppState {
            sys: Mutex::new(sys),
        }
    }

    /// Execute an arbitrary Command through the core runtime.
    pub fn execute(&self, cmd: Command) -> Response {
        let mut sys = self.sys.lock().unwrap();
        sys.execute(cmd)
    }

    /// Return the pending actions from the last execute() call.
    pub fn pending_actions(&self) -> Vec<Action> {
        let sys = self.sys.lock().unwrap();
        sys.pending_actions().to_vec()
    }

    /// Drain and return accumulated actions.
    pub fn drain_actions(&self) -> Vec<Action> {
        let mut sys = self.sys.lock().unwrap();
        sys.drain_actions()
    }

    // -------------------------------------------------------------------
    // Top-level commands
    // -------------------------------------------------------------------

    pub fn status(&self) -> Response {
        self.execute(Command::Status { format: None })
    }

    pub fn view(&self, name: String) -> Response {
        self.execute(Command::View { name })
    }

    pub fn help(&self, topic: Option<String>) -> Response {
        self.execute(Command::Help { topic })
    }

    // -------------------------------------------------------------------
    // Layout commands
    // -------------------------------------------------------------------

    pub fn layout_row(&self, session: String, percent: Option<String>) -> Response {
        self.execute(Command::LayoutRow { session, percent })
    }

    pub fn layout_column(&self, session: String, percent: Option<String>) -> Response {
        self.execute(Command::LayoutColumn { session, percent })
    }

    pub fn layout_merge(&self, session: String) -> Response {
        self.execute(Command::LayoutMerge { session })
    }

    pub fn layout_place(&self, pane: String, agent: String) -> Response {
        self.execute(Command::LayoutPlace { pane, agent })
    }

    pub fn layout_capture(&self, session: String) -> Response {
        self.execute(Command::LayoutCapture { session })
    }

    pub fn layout_session(&self, name: String, cwd: Option<String>) -> Response {
        self.execute(Command::LayoutSession { name, cwd })
    }

    // -------------------------------------------------------------------
    // Client commands
    // -------------------------------------------------------------------

    pub fn client_next(&self) -> Response {
        self.execute(Command::ClientNext)
    }

    pub fn client_prev(&self) -> Response {
        self.execute(Command::ClientPrev)
    }
}


/// Assemble and run the Tauri application.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let project_root = std::env::var("MUX_PROJECT_ROOT").unwrap_or_default();
    let state = AppState::new(project_root);
    let overlay_state = OverlayState::new();
    let overlay_args = OverlayArgs::from_env();

    tauri::Builder::default()
        .manage(state)
        .manage(overlay_state)
        .invoke_handler(tauri::generate_handler![
            // Top-level
            ipc::mux_status,
            ipc::mux_view,
            ipc::mux_help,
            // Layout
            ipc::mux_layout_row,
            ipc::mux_layout_column,
            ipc::mux_layout_merge,
            ipc::mux_layout_place,
            ipc::mux_layout_capture,
            ipc::mux_layout_session,
            // Client
            ipc::mux_client_next,
            ipc::mux_client_prev,
            // Overlay
            ipc::mux_show_overlay,
            ipc::mux_hide_overlay,
            ipc::mux_get_target_pane,
        ])
        .setup(move |app| {
            if let Some(args) = overlay_args {
                if let Some(window) = app.get_webview_window("main") {
                    let overlay: tauri::State<OverlayState> = app.state();
                    overlay.show(args.pane);

                    let _ = window.set_position(tauri::PhysicalPosition::new(
                        args.x, args.y,
                    ));
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}


#[cfg(test)]
mod tests {
    use super::*;
    use cmx_utils::response::Action;

    fn test_state() -> AppState {
        AppState::new("/tmp".into())
    }

    fn is_ok(r: &Response) -> bool {
        matches!(r, Response::Ok { .. })
    }

    fn output(r: &Response) -> &str {
        match r {
            Response::Ok { output } => output,
            Response::Error { message } => message,
        }
    }

    #[test]
    fn status_ok() {
        let state = test_state();
        let r = state.status();
        assert!(is_ok(&r));
    }

    #[test]
    fn view_returns_name() {
        let state = test_state();
        let r = state.view("main".into());
        assert!(is_ok(&r));
        assert!(output(&r).contains("main"));
    }

    #[test]
    fn help_overview() {
        let state = test_state();
        let r = state.help(None);
        assert!(is_ok(&r));
        assert!(output(&r).contains("mux"));
    }

    #[test]
    fn layout_row() {
        let state = test_state();
        let r = state.layout_row("main".into(), Some("30".into()));
        assert!(is_ok(&r));
        let actions = state.pending_actions();
        assert_eq!(actions.len(), 1);
    }

    #[test]
    fn layout_column() {
        let state = test_state();
        let r = state.layout_column("main".into(), None);
        assert!(is_ok(&r));
    }

    #[test]
    fn layout_merge() {
        let state = test_state();
        let r = state.layout_merge("main".into());
        assert!(is_ok(&r));
        assert!(output(&r).contains("Merge queued"));
    }

    #[test]
    fn layout_place() {
        let state = test_state();
        let r = state.layout_place("%3".into(), "w1".into());
        assert!(is_ok(&r));
    }

    #[test]
    fn layout_capture() {
        let state = test_state();
        let r = state.layout_capture("main".into());
        assert!(is_ok(&r));
        assert!(output(&r).contains("Capture queued"));
    }

    #[test]
    fn layout_session_with_cwd() {
        let state = test_state();
        let r = state.layout_session("work".into(), Some("/tmp".into()));
        assert!(is_ok(&r));
        let actions = state.pending_actions();
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            Action::CreateSession { name, cwd } => {
                assert_eq!(name, "work");
                assert_eq!(cwd, "/tmp");
            }
            other => panic!("Expected CreateSession, got {:?}", other),
        }
    }

    #[test]
    fn client_next_and_prev() {
        let state = test_state();
        let r = state.client_next();
        assert!(is_ok(&r));
        let r = state.client_prev();
        assert!(is_ok(&r));
    }

    #[test]
    fn overlay_starts_hidden() {
        let overlay = OverlayState::new();
        assert!(!overlay.is_visible());
        assert!(overlay.get_target_pane().is_none());
    }

    #[test]
    fn overlay_show_sets_visible_and_pane() {
        let overlay = OverlayState::new();
        overlay.show("%42".into());
        assert!(overlay.is_visible());
        assert_eq!(overlay.get_target_pane(), Some("%42".into()));
    }

    #[test]
    fn overlay_hide_clears_visible_preserves_pane() {
        let overlay = OverlayState::new();
        overlay.show("%42".into());
        overlay.hide();
        assert!(!overlay.is_visible());
        assert_eq!(overlay.get_target_pane(), Some("%42".into()));
    }

    #[test]
    fn drain_actions_clears() {
        let state = test_state();
        state.layout_row("main".into(), None);
        let drained = state.drain_actions();
        assert_eq!(drained.len(), 1);
    }

    #[test]
    fn actions_cleared_between_executes() {
        let state = test_state();
        state.layout_row("main".into(), None);
        assert_eq!(state.pending_actions().len(), 1);
        state.status();
        assert!(state.pending_actions().is_empty());
    }

    #[test]
    fn concurrent_status_calls() {
        use std::sync::Arc;
        use std::thread;

        let state = Arc::new(test_state());
        let mut handles = Vec::new();

        for _ in 0..10 {
            let s = Arc::clone(&state);
            handles.push(thread::spawn(move || {
                let r = s.status();
                assert!(is_ok(&r));
            }));
        }

        for h in handles {
            h.join().unwrap();
        }
    }
}
