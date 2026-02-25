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


/// Menu item IDs used by the tray icon menu.
///
/// Exposed as constants so they can be tested and referenced consistently.
pub mod tray_menu_ids {
    pub const SHOW: &str = "show";
    pub const CONFIG: &str = "config";
    pub const HELP: &str = "help";
    pub const QUIT: &str = "quit";
}


/// Overlay window state, separate from core AppState.
///
/// Tracks the overlay visibility and the target tmux pane ID.
/// Also used by the tray icon's "Show MuxUX" menu item to toggle overlay.
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

    /// Toggle visibility. If currently visible, hide. If hidden, show with
    /// the given pane_id. Returns the new visibility state.
    pub fn toggle(&self, pane_id: String) -> bool {
        if self.is_visible() {
            self.hide();
            false
        } else {
            self.show(pane_id);
            true
        }
    }
}


/// Information about a tmux pane: its ID and character-grid position.
#[derive(Debug, Clone, PartialEq)]
pub struct TmuxPaneInfo {
    pub pane_id: String,
    pub left: u32,
    pub top: u32,
    pub width: u32,
    pub height: u32,
}


/// Query tmux for the current pane ID.
///
/// Runs `tmux display-message -p '#{pane_id}'` and returns the trimmed output,
/// or `None` if tmux is unavailable.
pub fn query_tmux_pane_id() -> Option<String> {
    let output = std::process::Command::new("tmux")
        .args(["display-message", "-p", "#{pane_id}"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let id = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if id.is_empty() { None } else { Some(id) }
}


/// Query tmux for the current pane's position and size in character cells.
///
/// Runs `tmux display-message -p '#{pane_id} #{pane_left} #{pane_top} #{pane_width} #{pane_height}'`
/// and parses the output.
pub fn query_tmux_pane_info() -> Option<TmuxPaneInfo> {
    let output = std::process::Command::new("tmux")
        .args([
            "display-message",
            "-p",
            "#{pane_id} #{pane_left} #{pane_top} #{pane_width} #{pane_height}",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_tmux_pane_info(&String::from_utf8_lossy(&output.stdout))
}


/// Parse tmux pane info from a string like `%42 0 0 80 24`.
pub fn parse_tmux_pane_info(s: &str) -> Option<TmuxPaneInfo> {
    let parts: Vec<&str> = s.trim().split_whitespace().collect();
    if parts.len() < 5 {
        return None;
    }
    Some(TmuxPaneInfo {
        pane_id: parts[0].to_string(),
        left: parts[1].parse().ok()?,
        top: parts[2].parse().ok()?,
        width: parts[3].parse().ok()?,
        height: parts[4].parse().ok()?,
    })
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

    /// Return frontend-relevant settings as a JSON string.
    pub fn get_settings(&self) -> String {
        let sys = self.sys.lock().unwrap();
        let s = sys.settings();
        serde_json::json!({
            "zone_max_width": s.zone_max_width,
            "search_max_rows": s.search_max_rows,
        })
        .to_string()
    }

    // -------------------------------------------------------------------
    // Top-level commands
    // -------------------------------------------------------------------

    pub fn status(&self) -> Response {
        self.execute(Command::Status { format: None })
    }

    pub fn session_list(&self) -> Response {
        self.execute(Command::SessionList)
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


/// Handle the global hotkey toggle: query tmux, then show/hide the overlay.
///
/// Called from the global shortcut handler and the tray icon "Show MuxUX" menu item.
fn hotkey_toggle_overlay(handle: &tauri::AppHandle) {
    let overlay: tauri::State<OverlayState> = handle.state();

    if overlay.is_visible() {
        eprintln!("[muxux] toggle: hiding overlay");
        overlay.hide();
        if let Some(window) = handle.get_webview_window("main") {
            let _ = window.hide();
        }
    } else {
        let pane_id = query_tmux_pane_id().unwrap_or_default();
        eprintln!("[muxux] toggle: showing overlay for pane '{}'", pane_id);
        overlay.show(pane_id);

        match handle.get_webview_window("main") {
            Some(window) => {
                if let Some(monitor) = window.current_monitor().ok().flatten() {
                    let size = monitor.size();
                    let pos = monitor.position();
                    let win_w = 400_i32;
                    let win_h = 400_i32;
                    let cx = pos.x + (size.width as i32 - win_w) / 2;
                    let cy = pos.y + (size.height as i32 - win_h) / 2;
                    eprintln!("[muxux] positioning at ({}, {}), monitor {}x{}", cx, cy, size.width, size.height);
                    let _ = window.set_position(tauri::PhysicalPosition::new(cx, cy));
                } else {
                    eprintln!("[muxux] WARNING: no monitor found");
                }
                match window.show() {
                    Ok(_) => eprintln!("[muxux] window.show() succeeded"),
                    Err(e) => eprintln!("[muxux] window.show() FAILED: {}", e),
                }
                match window.set_focus() {
                    Ok(_) => eprintln!("[muxux] window.set_focus() succeeded"),
                    Err(e) => eprintln!("[muxux] window.set_focus() FAILED: {}", e),
                }
            }
            None => eprintln!("[muxux] ERROR: no window named 'main' found"),
        }
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
            ipc::mux_session_list,
            ipc::mux_view,
            ipc::mux_help,
            // Settings
            ipc::mux_get_settings,
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
            ipc::mux_toggle_overlay,
        ])
        .setup(move |app| {
            // Handle CLI overlay args (existing right-click trigger path)
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

            // ---------------------------------------------------------------
            // Tray icon setup — persistent macOS menu bar icon (M8.2.4)
            // ---------------------------------------------------------------
            {
                use tauri::tray::TrayIconBuilder;
                use tauri::menu::{MenuBuilder, MenuItemBuilder};

                eprintln!("[muxux] setting up tray icon...");

                let show_item = MenuItemBuilder::with_id(
                    tray_menu_ids::SHOW, "Show MuxUX",
                ).build(app)?;
                let config_item = MenuItemBuilder::with_id(
                    tray_menu_ids::CONFIG, "Config",
                ).build(app)?;
                let help_item = MenuItemBuilder::with_id(
                    tray_menu_ids::HELP, "Help",
                ).build(app)?;
                let quit_item = MenuItemBuilder::with_id(
                    tray_menu_ids::QUIT, "Quit",
                ).build(app)?;

                let menu = MenuBuilder::new(app)
                    .item(&show_item)
                    .separator()
                    .item(&config_item)
                    .item(&help_item)
                    .separator()
                    .item(&quit_item)
                    .build()?;

                let handle_for_tray = app.handle().clone();
                let has_icon = app.default_window_icon().is_some();
                eprintln!("[muxux] default_window_icon present: {}", has_icon);

                let mut builder = TrayIconBuilder::new()
                    .title("MuxUX")
                    .tooltip("MuxUX — Structure App")
                    .menu(&menu);
                if let Some(icon) = app.default_window_icon().cloned() {
                    builder = builder.icon(icon);
                }
                let _tray = builder
                    .on_menu_event(move |_app, event| {
                        eprintln!("[muxux] tray menu event: {:?}", event.id());
                        match event.id().as_ref() {
                            tray_menu_ids::SHOW => {
                                hotkey_toggle_overlay(&handle_for_tray);
                            }
                            tray_menu_ids::QUIT => {
                                std::process::exit(0);
                            }
                            _ => {} // config, help — placeholder for now
                        }
                    })
                    .build(app)?;
            }

            // ---------------------------------------------------------------
            // Global hotkey: Ctrl+Shift+Space (all platforms)
            // ---------------------------------------------------------------
            #[cfg(desktop)]
            {
                use tauri_plugin_global_shortcut::{
                    Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState,
                };

                let shortcut = Shortcut::new(
                    Some(Modifiers::CONTROL | Modifiers::SHIFT),
                    Code::Space,
                );

                eprintln!("[muxux] registering global shortcut Ctrl+Shift+Space...");
                let handle = app.handle().clone();
                app.handle().plugin(
                    tauri_plugin_global_shortcut::Builder::new()
                        .with_handler(move |_app, fired, event| {
                            eprintln!("[muxux] shortcut event: {:?} state={:?}", fired, event.state());
                            if fired == &shortcut
                                && matches!(event.state(), ShortcutState::Pressed)
                            {
                                hotkey_toggle_overlay(&handle);
                            }
                        })
                        .build(),
                )?;

                match app.global_shortcut().register(shortcut) {
                    Ok(_) => eprintln!("[muxux] shortcut registered successfully"),
                    Err(e) => eprintln!("[muxux] shortcut registration FAILED: {}", e),
                }
            }

            eprintln!("[muxux] setup complete");
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
    fn session_list_returns_json_array() {
        let state = test_state();
        let r = state.session_list();
        assert!(is_ok(&r));
        // Output must be valid JSON array (may be empty if tmux unavailable)
        let parsed: serde_json::Value = serde_json::from_str(output(&r)).unwrap();
        assert!(parsed.is_array());
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
    fn overlay_toggle_show_hide_show() {
        let overlay = OverlayState::new();

        // Toggle from hidden -> visible
        let now_visible = overlay.toggle("%10".into());
        assert!(now_visible);
        assert!(overlay.is_visible());
        assert_eq!(overlay.get_target_pane(), Some("%10".into()));

        // Toggle from visible -> hidden
        let now_visible = overlay.toggle("%10".into());
        assert!(!now_visible);
        assert!(!overlay.is_visible());

        // Toggle from hidden -> visible again (with new pane)
        let now_visible = overlay.toggle("%20".into());
        assert!(now_visible);
        assert!(overlay.is_visible());
        assert_eq!(overlay.get_target_pane(), Some("%20".into()));
    }

    #[test]
    fn parse_tmux_pane_info_valid() {
        let info = parse_tmux_pane_info("%42 0 0 80 24").unwrap();
        assert_eq!(info.pane_id, "%42");
        assert_eq!(info.left, 0);
        assert_eq!(info.top, 0);
        assert_eq!(info.width, 80);
        assert_eq!(info.height, 24);
    }

    #[test]
    fn parse_tmux_pane_info_with_offset() {
        let info = parse_tmux_pane_info("%7 10 5 120 40").unwrap();
        assert_eq!(info.pane_id, "%7");
        assert_eq!(info.left, 10);
        assert_eq!(info.top, 5);
        assert_eq!(info.width, 120);
        assert_eq!(info.height, 40);
    }

    #[test]
    fn parse_tmux_pane_info_trailing_newline() {
        let info = parse_tmux_pane_info("%1 0 0 80 24\n").unwrap();
        assert_eq!(info.pane_id, "%1");
        assert_eq!(info.width, 80);
    }

    #[test]
    fn parse_tmux_pane_info_too_few_parts() {
        assert!(parse_tmux_pane_info("%42 0 0").is_none());
    }

    #[test]
    fn parse_tmux_pane_info_empty() {
        assert!(parse_tmux_pane_info("").is_none());
    }

    #[test]
    fn parse_tmux_pane_info_bad_number() {
        assert!(parse_tmux_pane_info("%42 x 0 80 24").is_none());
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

    // -------------------------------------------------------------------
    // Tray menu ID tests (M8.2.4)
    // -------------------------------------------------------------------

    #[test]
    fn tray_menu_ids_are_distinct() {
        let ids = [
            tray_menu_ids::SHOW,
            tray_menu_ids::CONFIG,
            tray_menu_ids::HELP,
            tray_menu_ids::QUIT,
        ];
        // All IDs must be unique
        for (i, a) in ids.iter().enumerate() {
            for (j, b) in ids.iter().enumerate() {
                if i != j {
                    assert_ne!(a, b, "tray menu IDs must be unique");
                }
            }
        }
    }

    #[test]
    fn tray_menu_ids_match_expected_strings() {
        assert_eq!(tray_menu_ids::SHOW, "show");
        assert_eq!(tray_menu_ids::CONFIG, "config");
        assert_eq!(tray_menu_ids::HELP, "help");
        assert_eq!(tray_menu_ids::QUIT, "quit");
    }

    #[test]
    fn tray_menu_ids_not_empty() {
        assert!(!tray_menu_ids::SHOW.is_empty());
        assert!(!tray_menu_ids::CONFIG.is_empty());
        assert!(!tray_menu_ids::HELP.is_empty());
        assert!(!tray_menu_ids::QUIT.is_empty());
    }

    // -------------------------------------------------------------------
    // Settings tests
    // -------------------------------------------------------------------

    #[test]
    fn get_settings_returns_valid_json() {
        let state = test_state();
        let json_str = state.get_settings();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert!(parsed.is_object());
    }

    #[test]
    fn get_settings_contains_expected_keys() {
        let state = test_state();
        let json_str = state.get_settings();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed["zone_max_width"], 160);
        assert_eq!(parsed["search_max_rows"], 10);
    }

    #[test]
    fn get_settings_only_frontend_fields() {
        let state = test_state();
        let json_str = state.get_settings();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        let obj = parsed.as_object().unwrap();
        // Should contain exactly the two frontend-relevant fields
        assert_eq!(obj.len(), 2);
        assert!(obj.contains_key("zone_max_width"));
        assert!(obj.contains_key("search_max_rows"));
    }
}
