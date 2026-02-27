//! Tauri IPC command handlers for MuxUX.
//!
//! Each function is a Tauri command that bridges the frontend JavaScript
//! to the core Sys runtime via AppState.

use crate::AppState;
use cmx_utils::response::Response;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU32, Ordering};
use tauri::State;


/// Uniform response type for all IPC commands.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IpcResponse {
    pub ok: bool,
    pub data: String,
}


impl IpcResponse {
    pub fn success(data: String) -> Self {
        IpcResponse { ok: true, data }
    }

    pub fn error(msg: String) -> Self {
        IpcResponse { ok: false, data: msg }
    }
}


fn to_ipc(resp: Response) -> IpcResponse {
    match resp {
        Response::Ok { output } => IpcResponse::success(output),
        Response::Error { message } => IpcResponse::error(message),
    }
}


// ---------------------------------------------------------------------------
// Top-level commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn mux_status(state: State<'_, AppState>) -> IpcResponse {
    to_ipc(state.status())
}

#[tauri::command]
pub fn mux_session_list(state: State<'_, AppState>) -> IpcResponse {
    to_ipc(state.session_list())
}

#[tauri::command]
pub fn mux_view(state: State<'_, AppState>, name: String) -> IpcResponse {
    to_ipc(state.view(name))
}

#[tauri::command]
pub fn mux_help(state: State<'_, AppState>, topic: Option<String>) -> IpcResponse {
    to_ipc(state.help(topic))
}


// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn mux_get_settings(state: State<'_, AppState>) -> IpcResponse {
    IpcResponse::success(state.get_settings())
}


// ---------------------------------------------------------------------------
// Layout commands
// ---------------------------------------------------------------------------

/// Return the overlay's target pane if non-empty, otherwise `"."` which tells
/// tmux to use "the current pane".
fn target_pane_or_current(overlay: &crate::OverlayState) -> String {
    match overlay.get_target_pane() {
        Some(ref p) if !p.is_empty() => p.clone(),
        _ => ".".to_string(),
    }
}

/// Resolve the target for a layout operation: if "current", use the overlay's
/// target pane; otherwise use the given value directly.
fn resolve_target(session: &str, overlay: &crate::OverlayState) -> String {
    if session == "current" {
        target_pane_or_current(overlay)
    } else {
        session.to_string()
    }
}

#[tauri::command]
pub fn mux_layout_row(
    state: State<'_, AppState>,
    overlay: State<'_, crate::OverlayState>,
    session: String,
    percent: Option<String>,
) -> IpcResponse {
    let target = resolve_target(&session, &overlay);
    eprintln!("[muxux-ipc] mux_layout_row: target={}", target);
    let resp = to_ipc(state.layout_row(target, percent));
    state.run_pending_actions();
    resp
}

#[tauri::command]
pub fn mux_layout_column(
    state: State<'_, AppState>,
    overlay: State<'_, crate::OverlayState>,
    session: String,
    percent: Option<String>,
) -> IpcResponse {
    let target = resolve_target(&session, &overlay);
    eprintln!("[muxux-ipc] mux_layout_column: target={}", target);
    let resp = to_ipc(state.layout_column(target, percent));
    state.run_pending_actions();
    resp
}

#[tauri::command]
pub fn mux_layout_merge(
    state: State<'_, AppState>,
    overlay: State<'_, crate::OverlayState>,
    session: String,
) -> IpcResponse {
    let target = resolve_target(&session, &overlay);
    eprintln!("[muxux-ipc] mux_layout_merge: target={}", target);
    let resp = to_ipc(state.layout_merge(target));
    state.run_pending_actions();
    resp
}

#[tauri::command]
pub fn mux_layout_place(
    state: State<'_, AppState>,
    pane: String,
    agent: String,
) -> IpcResponse {
    let resp = to_ipc(state.layout_place(pane, agent));
    state.run_pending_actions();
    resp
}

#[tauri::command]
pub fn mux_layout_capture(state: State<'_, AppState>, session: String) -> IpcResponse {
    to_ipc(state.layout_capture(session))
}

#[tauri::command]
pub fn mux_layout_session(
    state: State<'_, AppState>,
    name: String,
    cwd: Option<String>,
) -> IpcResponse {
    let resp = to_ipc(state.layout_session(name, cwd));
    state.run_pending_actions();
    resp
}

// ---------------------------------------------------------------------------
// Direct tmux operations (Phase 1)
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn mux_layout_resize(
    state: State<'_, AppState>,
    overlay: State<'_, crate::OverlayState>,
    direction: String,
    amount: Option<u32>,
) -> IpcResponse {
    let pane = target_pane_or_current(&overlay);
    eprintln!("[muxux-ipc] mux_layout_resize: pane={} dir={}", pane, direction);
    to_ipc(state.layout_resize(&pane, &direction, amount.unwrap_or(5)))
}

#[tauri::command]
pub fn mux_layout_even_out(
    state: State<'_, AppState>,
    overlay: State<'_, crate::OverlayState>,
) -> IpcResponse {
    let pane = target_pane_or_current(&overlay);
    eprintln!("[muxux-ipc] mux_layout_even_out: pane={}", pane);
    to_ipc(state.layout_even_out(&pane))
}

#[tauri::command]
pub fn mux_layout_kill_pane(
    state: State<'_, AppState>,
    overlay: State<'_, crate::OverlayState>,
) -> IpcResponse {
    let pane = target_pane_or_current(&overlay);
    eprintln!("[muxux-ipc] mux_layout_kill_pane: pane={}", pane);
    to_ipc(state.layout_kill_pane(&pane))
}

#[tauri::command]
pub fn mux_layout_swap_pane(
    state: State<'_, AppState>,
    overlay: State<'_, crate::OverlayState>,
    direction: String,
) -> IpcResponse {
    let pane = target_pane_or_current(&overlay);
    eprintln!("[muxux-ipc] mux_layout_swap_pane: pane={} dir={}", pane, direction);
    to_ipc(state.layout_swap_pane(&pane, &direction))
}

#[tauri::command]
pub fn mux_layout_break_pane(
    state: State<'_, AppState>,
    overlay: State<'_, crate::OverlayState>,
) -> IpcResponse {
    let pane = target_pane_or_current(&overlay);
    eprintln!("[muxux-ipc] mux_layout_break_pane: pane={}", pane);
    to_ipc(state.layout_break_pane(&pane))
}

// ---------------------------------------------------------------------------
// Client commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn mux_client_next(state: State<'_, AppState>) -> IpcResponse {
    to_ipc(state.client_next())
}

#[tauri::command]
pub fn mux_client_prev(state: State<'_, AppState>) -> IpcResponse {
    to_ipc(state.client_prev())
}

// ---------------------------------------------------------------------------
// Session switch (Phase 2)
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn mux_session_switch(
    state: State<'_, AppState>,
    name: String,
) -> IpcResponse {
    to_ipc(state.session_switch(&name))
}

// ---------------------------------------------------------------------------
// Template application (Phase 3)
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn mux_template_apply(
    state: State<'_, AppState>,
    overlay: State<'_, crate::OverlayState>,
    template: String,
) -> IpcResponse {
    let pane = target_pane_or_current(&overlay);
    eprintln!("[muxux-ipc] mux_template_apply: pane={} template={}", pane, template);
    to_ipc(state.template_apply(&pane, &template))
}


// ---------------------------------------------------------------------------
// Layout capture (Phase 5)
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn mux_layout_capture_live(
    state: State<'_, AppState>,
    overlay: State<'_, crate::OverlayState>,
    session: Option<String>,
) -> IpcResponse {
    // Use overlay target pane's session, or the provided session name
    let target = session.unwrap_or_else(|| {
        overlay
            .get_target_pane()
            .unwrap_or_else(|| "0".to_string())
    });
    to_ipc(state.layout_capture_live(&target))
}

#[tauri::command]
pub fn mux_layout_capture_save(
    state: State<'_, AppState>,
    overlay: State<'_, crate::OverlayState>,
    name: String,
    session: Option<String>,
) -> IpcResponse {
    let target = session.unwrap_or_else(|| {
        overlay
            .get_target_pane()
            .unwrap_or_else(|| "0".to_string())
    });
    to_ipc(state.layout_capture_save(&target, &name))
}

// ---------------------------------------------------------------------------
// Parts catalog (Phase 4)
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn mux_parts_list(state: State<'_, AppState>) -> IpcResponse {
    to_ipc(state.parts_list())
}

#[tauri::command]
pub fn mux_parts_place(
    state: State<'_, AppState>,
    overlay: State<'_, crate::OverlayState>,
    part: String,
) -> IpcResponse {
    let pane = target_pane_or_current(&overlay);
    eprintln!("[muxux-ipc] mux_parts_place: pane={} part={}", pane, part);
    to_ipc(state.parts_place(&pane, &part))
}

// ---------------------------------------------------------------------------
// Overlay commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn mux_show_overlay(
    window: tauri::WebviewWindow,
    overlay: State<'_, crate::OverlayState>,
    x: i32,
    y: i32,
    pane_id: String,
) -> IpcResponse {
    overlay.show(pane_id.clone());
    eprintln!("[muxux-ipc] mux_show_overlay: positioning at ({},{}) for pane {}", x, y, pane_id);
    let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
    let _ = window.show();
    let _ = window.set_focus();
    IpcResponse::success(format!("overlay shown at ({}, {}) for pane {}", x, y, pane_id))
}

#[tauri::command]
pub fn mux_hide_overlay(
    window: tauri::WebviewWindow,
    overlay: State<'_, crate::OverlayState>,
) -> IpcResponse {
    overlay.hide();
    let _ = window.hide();
    IpcResponse::success("overlay hidden".into())
}

#[tauri::command]
pub fn mux_get_target_pane(
    overlay: State<'_, crate::OverlayState>,
) -> IpcResponse {
    match overlay.get_target_pane() {
        Some(pane) => IpcResponse::success(pane),
        None => IpcResponse::error("no target pane set".into()),
    }
}

/// Toggle the overlay: if visible, hide it; if hidden, query tmux and show it.
///
/// This provides the same toggle behavior as the global hotkey but accessible
/// from the frontend via IPC.
#[tauri::command]
pub fn mux_toggle_overlay(
    window: tauri::WebviewWindow,
    overlay: State<'_, crate::OverlayState>,
) -> IpcResponse {
    if overlay.is_visible() {
        overlay.hide();
        let _ = window.hide();
        IpcResponse::success("overlay hidden".into())
    } else {
        let pane_id = crate::query_tmux_pane_id().unwrap_or_else(|| ".".to_string());
        eprintln!("[muxux-ipc] mux_toggle_overlay: pane_id={}", pane_id);
        overlay.show(pane_id.clone());

        // Center on cursor via CoreGraphics
        let half = crate::OVERLAY_SIZE / 2;
        if let Some((mx, my)) = crate::get_mouse_position() {
            let _ = window.set_position(tauri::PhysicalPosition::new(mx - half, my - half));
        } else if let Some(monitor) = window.current_monitor().ok().flatten() {
            let size = monitor.size();
            let pos = monitor.position();
            let cx = pos.x + (size.width as i32 - crate::OVERLAY_SIZE) / 2;
            let cy = pos.y + (size.height as i32 - crate::OVERLAY_SIZE) / 2;
            let _ = window.set_position(tauri::PhysicalPosition::new(cx, cy));
        }
        let _ = window.show();
        let _ = window.set_focus();
        IpcResponse::success(format!("overlay shown for pane {}", pane_id))
    }
}


/// Summon the overlay from any window (e.g. a terminal window) at given screen coords.
///
/// Unlike `mux_show_overlay` which operates on the calling window, this finds
/// the "main" overlay window by label and shows it at (x, y).
#[tauri::command]
pub fn mux_summon_overlay(
    app: tauri::AppHandle,
    overlay: State<'_, crate::OverlayState>,
    x: i32,
    y: i32,
) -> IpcResponse {
    use tauri::Manager;
    let pane_id = crate::query_tmux_pane_id().unwrap_or_else(|| ".".to_string());
    eprintln!("[muxux-ipc] mux_summon_overlay: pane_id={} at ({},{})", pane_id, x, y);
    overlay.show(pane_id.clone());

    match app.get_webview_window("main") {
        Some(window) => {
            let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
            let _ = window.show();
            let _ = window.set_focus();
            IpcResponse::success(format!("overlay summoned at ({}, {}) for pane {}", x, y, pane_id))
        }
        None => IpcResponse::error("overlay window 'main' not found".into()),
    }
}


// ---------------------------------------------------------------------------
// Terminal commands
// ---------------------------------------------------------------------------

static TERMINAL_COUNTER: AtomicU32 = AtomicU32::new(0);

#[tauri::command]
pub fn mux_open_terminal(app: tauri::AppHandle) -> IpcResponse {
    let n = TERMINAL_COUNTER.fetch_add(1, Ordering::Relaxed);
    let label = format!("terminal-{}", n);

    let url = crate::terminal_url();
    match tauri::WebviewWindowBuilder::new(&app, &label, url)
        .title("MuxUX Terminal")
        .inner_size(900.0, 600.0)
        .resizable(true)
        .decorations(true)
        .always_on_top(false)
        .build()
    {
        Ok(_) => IpcResponse::success(format!("terminal window '{}' opened", label)),
        Err(e) => IpcResponse::error(format!("failed to open terminal: {}", e)),
    }
}


// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipc_response_success() {
        let r = IpcResponse::success("hello".into());
        assert!(r.ok);
        assert_eq!(r.data, "hello");
    }

    #[test]
    fn ipc_response_error() {
        let r = IpcResponse::error("not found".into());
        assert!(!r.ok);
        assert_eq!(r.data, "not found");
    }

    #[test]
    fn to_ipc_ok() {
        let resp = Response::Ok {
            output: "running".into(),
        };
        let ipc = to_ipc(resp);
        assert!(ipc.ok);
        assert_eq!(ipc.data, "running");
    }

    #[test]
    fn to_ipc_error() {
        let resp = Response::Error {
            message: "not found".into(),
        };
        let ipc = to_ipc(resp);
        assert!(!ipc.ok);
        assert_eq!(ipc.data, "not found");
    }

    #[test]
    fn ipc_response_serde_round_trip() {
        let r = IpcResponse::success("test data".into());
        let json = serde_json::to_string(&r).unwrap();
        let back: IpcResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(back, r);
    }

    #[test]
    fn ipc_response_json_shape_ok() {
        let r = IpcResponse::success("output".into());
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("\"ok\":true"));
        assert!(json.contains("\"data\":\"output\""));
    }

    #[test]
    fn ipc_response_json_shape_error() {
        let r = IpcResponse::error("bad request".into());
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("\"ok\":false"));
        assert!(json.contains("\"data\":\"bad request\""));
    }

    #[test]
    fn ipc_response_empty_data() {
        let r = IpcResponse::success(String::new());
        assert!(r.ok);
        assert!(r.data.is_empty());
    }

    #[test]
    fn ipc_response_equality() {
        let r1 = IpcResponse::success("ok".into());
        let r2 = IpcResponse::success("ok".into());
        let r3 = IpcResponse::error("ok".into());
        assert_eq!(r1, r2);
        assert_ne!(r1, r3);
    }

    #[test]
    fn ipc_response_settings_json_round_trip() {
        // Verify that a settings JSON payload survives IpcResponse serialization
        let settings_json = r#"{"zone_max_width":160,"search_max_rows":10,"terminal":"muxux","lr_slide_start":5,"lr_slide_full":40,"color_scheme":"system"}"#;
        let r = IpcResponse::success(settings_json.into());
        let serialized = serde_json::to_string(&r).unwrap();
        let back: IpcResponse = serde_json::from_str(&serialized).unwrap();
        assert!(back.ok);
        // Parse the inner data as JSON to verify structure
        let inner: serde_json::Value = serde_json::from_str(&back.data).unwrap();
        assert_eq!(inner["zone_max_width"], 160);
        assert_eq!(inner["search_max_rows"], 10);
        assert_eq!(inner["terminal"], "muxux");
        assert_eq!(inner["lr_slide_start"], 5);
        assert_eq!(inner["lr_slide_full"], 40);
        assert_eq!(inner["color_scheme"], "system");
    }
}
