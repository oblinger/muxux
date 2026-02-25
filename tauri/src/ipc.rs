//! Tauri IPC command handlers for MuxUX.
//!
//! Each function is a Tauri command that bridges the frontend JavaScript
//! to the core Sys runtime via AppState.

use crate::AppState;
use cmx_utils::response::Response;
use serde::{Deserialize, Serialize};
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

#[tauri::command]
pub fn mux_layout_row(
    state: State<'_, AppState>,
    session: String,
    percent: Option<String>,
) -> IpcResponse {
    to_ipc(state.layout_row(session, percent))
}

#[tauri::command]
pub fn mux_layout_column(
    state: State<'_, AppState>,
    session: String,
    percent: Option<String>,
) -> IpcResponse {
    to_ipc(state.layout_column(session, percent))
}

#[tauri::command]
pub fn mux_layout_merge(state: State<'_, AppState>, session: String) -> IpcResponse {
    to_ipc(state.layout_merge(session))
}

#[tauri::command]
pub fn mux_layout_place(
    state: State<'_, AppState>,
    pane: String,
    agent: String,
) -> IpcResponse {
    to_ipc(state.layout_place(pane, agent))
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
    to_ipc(state.layout_session(name, cwd))
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
        let pane_id = crate::query_tmux_pane_id().unwrap_or_default();
        overlay.show(pane_id.clone());

        // Phase 1: center on screen
        if let Some(monitor) = window.current_monitor().ok().flatten() {
            let size = monitor.size();
            let pos = monitor.position();
            let win_w = 400_i32;
            let win_h = 400_i32;
            let cx = pos.x + (size.width as i32 - win_w) / 2;
            let cy = pos.y + (size.height as i32 - win_h) / 2;
            let _ = window.set_position(tauri::PhysicalPosition::new(cx, cy));
        }
        let _ = window.show();
        let _ = window.set_focus();
        IpcResponse::success(format!("overlay shown for pane {}", pane_id))
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
        let settings_json = r#"{"zone_max_width":160,"search_max_rows":10}"#;
        let r = IpcResponse::success(settings_json.into());
        let serialized = serde_json::to_string(&r).unwrap();
        let back: IpcResponse = serde_json::from_str(&serialized).unwrap();
        assert!(back.ok);
        // Parse the inner data as JSON to verify structure
        let inner: serde_json::Value = serde_json::from_str(&back.data).unwrap();
        assert_eq!(inner["zone_max_width"], 160);
        assert_eq!(inner["search_max_rows"], 10);
    }
}
