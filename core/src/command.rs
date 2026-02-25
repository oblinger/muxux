//! Command — the typed interface for all MuxUX operations.
//!
//! MuxUX handles layout, session, and structure commands. Docket operations
//! (agent, task, config, etc.) are handled by the skill-docket-app.

use serde::{Deserialize, Serialize};


#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "command")]
pub enum Command {
    // -----------------------------------------------------------------
    // Top-level commands
    // -----------------------------------------------------------------

    #[serde(rename = "status")]
    Status {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        format: Option<String>,
    },

    #[serde(rename = "session.list")]
    SessionList,

    #[serde(rename = "view")]
    View {
        name: String,
    },

    // -----------------------------------------------------------------
    // Layout commands
    // -----------------------------------------------------------------

    #[serde(rename = "layout.row")]
    LayoutRow {
        session: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        percent: Option<String>,
    },

    #[serde(rename = "layout.column")]
    LayoutColumn {
        session: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        percent: Option<String>,
    },

    #[serde(rename = "layout.merge")]
    LayoutMerge {
        session: String,
    },

    #[serde(rename = "layout.place")]
    LayoutPlace {
        pane: String,
        agent: String,
    },

    #[serde(rename = "layout.capture")]
    LayoutCapture {
        session: String,
    },

    #[serde(rename = "layout.session")]
    LayoutSession {
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cwd: Option<String>,
    },

    // -----------------------------------------------------------------
    // Client commands
    // -----------------------------------------------------------------

    #[serde(rename = "client.next")]
    ClientNext,

    #[serde(rename = "client.prev")]
    ClientPrev,

    // -----------------------------------------------------------------
    // Watch / Daemon / Help
    // -----------------------------------------------------------------

    #[serde(rename = "watch")]
    Watch {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        since: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        timeout: Option<String>,
    },

    #[serde(rename = "daemon.run")]
    DaemonRun,

    #[serde(rename = "daemon.stop")]
    DaemonStop,

    #[serde(rename = "help")]
    Help {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        topic: Option<String>,
    },

    // -----------------------------------------------------------------
    // Overlay / Hook commands
    // -----------------------------------------------------------------

    #[serde(rename = "studio")]
    Studio {
        pane: String,
        x: u32,
        y: u32,
    },

    #[serde(rename = "setup.hook")]
    SetupHook,

    #[serde(rename = "setup.unhook")]
    RemoveHook,
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_round_trip() {
        let cmd = Command::Status { format: None };
        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains("\"command\":\"status\""));
        let back: Command = serde_json::from_str(&json).unwrap();
        assert_eq!(back, cmd);
    }

    #[test]
    fn session_list_round_trip() {
        let cmd = Command::SessionList;
        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains("\"command\":\"session.list\""));
        let back: Command = serde_json::from_str(&json).unwrap();
        assert_eq!(back, cmd);
    }

    #[test]
    fn layout_row_round_trip() {
        let cmd = Command::LayoutRow {
            session: "main".into(),
            percent: Some("60".into()),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains("\"command\":\"layout.row\""));
        let back: Command = serde_json::from_str(&json).unwrap();
        assert_eq!(back, cmd);
    }

    #[test]
    fn layout_session_round_trip() {
        let cmd = Command::LayoutSession {
            name: "work".into(),
            cwd: Some("/tmp".into()),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains("\"command\":\"layout.session\""));
        let back: Command = serde_json::from_str(&json).unwrap();
        assert_eq!(back, cmd);
    }

    #[test]
    fn client_next_round_trip() {
        let cmd = Command::ClientNext;
        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains("\"command\":\"client.next\""));
        let back: Command = serde_json::from_str(&json).unwrap();
        assert_eq!(back, cmd);
    }

    #[test]
    fn studio_round_trip() {
        let cmd = Command::Studio {
            pane: "%1".into(),
            x: 50,
            y: 30,
        };
        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains("\"command\":\"studio\""));
        let back: Command = serde_json::from_str(&json).unwrap();
        assert_eq!(back, cmd);
    }

    #[test]
    fn setup_hook_round_trip() {
        let cmd = Command::SetupHook;
        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains("\"command\":\"setup.hook\""));
        let back: Command = serde_json::from_str(&json).unwrap();
        assert_eq!(back, cmd);
    }

    #[test]
    fn remove_hook_round_trip() {
        let cmd = Command::RemoveHook;
        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains("\"command\":\"setup.unhook\""));
        let back: Command = serde_json::from_str(&json).unwrap();
        assert_eq!(back, cmd);
    }
}
