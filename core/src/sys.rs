use crate::command::Command;
use crate::infrastructure::tmux::{TmuxCommandBuilder, parse_list_sessions};
use crate::types::config::MuxSettings;
use cmx_utils::response::{Action, Direction, Response};


/// Central runtime for MuxUX. Dispatches layout, session, and structure commands.
pub struct Sys {
    project_root: String,
    actions: Vec<Action>,
    settings: MuxSettings,
}


impl Sys {
    pub fn new(project_root: String) -> Sys {
        let settings = MuxSettings {
            project_root: project_root.clone(),
            ..MuxSettings::default()
        };
        Sys {
            project_root,
            actions: Vec::new(),
            settings,
        }
    }

    /// Return a reference to the current settings.
    pub fn settings(&self) -> &MuxSettings {
        &self.settings
    }

    /// The single dispatch method.
    pub fn execute(&mut self, cmd: Command) -> Response {
        self.actions.clear();
        match cmd {
            Command::Status { format } => self.cmd_status(format),
            Command::SessionList => self.cmd_session_list(),
            Command::View { name } => self.cmd_view(name),
            Command::LayoutRow { session, percent } => self.cmd_layout_row(session, percent),
            Command::LayoutColumn { session, percent } => self.cmd_layout_column(session, percent),
            Command::LayoutMerge { session } => self.cmd_layout_merge(session),
            Command::LayoutPlace { pane, agent } => self.cmd_layout_place(pane, agent),
            Command::LayoutCapture { session } => self.cmd_layout_capture(session),
            Command::LayoutSession { name, cwd } => self.cmd_layout_session(name, cwd),
            Command::ClientNext => self.cmd_client_next(),
            Command::ClientPrev => self.cmd_client_prev(),
            Command::Watch { .. } => Response::Error {
                message: "Watch commands are handled at the service layer".into(),
            },
            Command::DaemonRun => Response::Error {
                message: "DaemonRun must be handled by the binary".into(),
            },
            Command::DaemonStop => Response::Ok {
                output: "MuxUX daemon shutting down".into(),
            },
            Command::Help { topic } => self.cmd_help(topic),
            Command::Studio { pane, x, y } => self.cmd_studio(pane, x, y),
            Command::SetupHook => self.cmd_setup_hook(),
            Command::RemoveHook => self.cmd_remove_hook(),
        }
    }

    /// Actions emitted during the last execute() call.
    pub fn pending_actions(&self) -> &[Action] {
        &self.actions
    }

    /// Take and clear accumulated actions.
    pub fn drain_actions(&mut self) -> Vec<Action> {
        std::mem::take(&mut self.actions)
    }

    // -----------------------------------------------------------------------
    // Status / View
    // -----------------------------------------------------------------------

    fn cmd_status(&self, _format: Option<String>) -> Response {
        Response::Ok {
            output: "MuxUX status: running".into(),
        }
    }

    fn cmd_session_list(&self) -> Response {
        let output = std::process::Command::new("tmux")
            .args(["list-sessions", "-F", "#{session_name}"])
            .output();
        match output {
            Ok(out) if out.status.success() => {
                let raw = String::from_utf8_lossy(&out.stdout);
                let names = parse_list_sessions(&raw);
                let json_array: Vec<serde_json::Value> = names
                    .into_iter()
                    .map(|n| serde_json::json!({ "name": n }))
                    .collect();
                Response::Ok {
                    output: serde_json::Value::Array(json_array).to_string(),
                }
            }
            Ok(_) => Response::Ok {
                output: "[]".into(),
            },
            Err(_) => Response::Ok {
                output: "[]".into(),
            },
        }
    }

    fn cmd_view(&self, name: String) -> Response {
        Response::Ok {
            output: format!("MuxUX view: {}", name),
        }
    }

    // -----------------------------------------------------------------------
    // Layout commands
    // -----------------------------------------------------------------------

    fn cmd_layout_row(&mut self, session: String, percent: Option<String>) -> Response {
        let percent = percent
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(50);
        self.actions.push(Action::SplitPane {
            session,
            direction: Direction::Horizontal,
            percent,
        });
        Response::Ok {
            output: "Row split queued".into(),
        }
    }

    fn cmd_layout_column(&mut self, session: String, percent: Option<String>) -> Response {
        let percent = percent
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(50);
        self.actions.push(Action::SplitPane {
            session,
            direction: Direction::Vertical,
            percent,
        });
        Response::Ok {
            output: "Column split queued".into(),
        }
    }

    fn cmd_layout_merge(&self, _session: String) -> Response {
        Response::Ok {
            output: "Merge queued".into(),
        }
    }

    fn cmd_layout_place(&mut self, pane_id: String, agent: String) -> Response {
        self.actions.push(Action::PlaceAgent {
            pane_id,
            agent: agent.clone(),
        });
        Response::Ok {
            output: format!("Agent '{}' placed in pane", agent),
        }
    }

    fn cmd_layout_capture(&mut self, session: String) -> Response {
        let builder = TmuxCommandBuilder::new();
        let list_cmd = builder.list_panes(&session);
        Response::Ok {
            output: format!("Capture queued: {}", list_cmd),
        }
    }

    fn cmd_layout_session(&mut self, name: String, cwd: Option<String>) -> Response {
        let cwd = cwd.unwrap_or_else(|| self.project_root.clone());
        self.actions.push(Action::CreateSession {
            name: name.clone(),
            cwd,
        });
        Response::Ok {
            output: format!("Session '{}' creation queued", name),
        }
    }

    // -----------------------------------------------------------------------
    // Client commands
    // -----------------------------------------------------------------------

    fn cmd_client_next(&self) -> Response {
        Response::Ok {
            output: "Switched to next client".into(),
        }
    }

    fn cmd_client_prev(&self) -> Response {
        Response::Ok {
            output: "Switched to previous client".into(),
        }
    }

    // -----------------------------------------------------------------------
    // Overlay / Hook commands
    // -----------------------------------------------------------------------

    fn cmd_studio(&self, pane: String, x: u32, y: u32) -> Response {
        Response::Ok {
            output: format!("overlay target: pane={} x={} y={}", pane, x, y),
        }
    }

    fn cmd_setup_hook(&self) -> Response {
        let builder = TmuxCommandBuilder::new();
        Response::Ok {
            output: builder.bind_mouse_hook("mux"),
        }
    }

    fn cmd_remove_hook(&self) -> Response {
        let builder = TmuxCommandBuilder::new();
        Response::Ok {
            output: builder.unbind_mouse_hook(),
        }
    }

    // -----------------------------------------------------------------------
    // Help
    // -----------------------------------------------------------------------

    fn cmd_help(&self, topic: Option<String>) -> Response {
        Response::Ok {
            output: crate::help::help_text(topic.as_deref()),
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_list_returns_json_array() {
        let mut sys = Sys::new("/tmp".into());
        let resp = sys.execute(Command::SessionList);
        match resp {
            Response::Ok { output } => {
                // Output must be valid JSON array
                let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
                assert!(parsed.is_array());
                // Each element (if any) must have a "name" field
                for entry in parsed.as_array().unwrap() {
                    assert!(entry.get("name").is_some());
                    assert!(entry["name"].is_string());
                }
            }
            Response::Error { message } => panic!("Unexpected error: {}", message),
        }
    }

    #[test]
    fn layout_row_emits_action() {
        let mut sys = Sys::new("/tmp".into());
        let resp = sys.execute(Command::LayoutRow {
            session: "main".into(),
            percent: Some("60".into()),
        });
        assert!(matches!(resp, Response::Ok { .. }));
        assert_eq!(sys.pending_actions().len(), 1);
    }

    #[test]
    fn layout_session_emits_create() {
        let mut sys = Sys::new("/tmp".into());
        let resp = sys.execute(Command::LayoutSession {
            name: "work".into(),
            cwd: None,
        });
        assert!(matches!(resp, Response::Ok { .. }));
        assert_eq!(sys.pending_actions().len(), 1);
    }

    #[test]
    fn client_next_ok() {
        let mut sys = Sys::new("/tmp".into());
        let resp = sys.execute(Command::ClientNext);
        assert!(matches!(resp, Response::Ok { .. }));
    }

    #[test]
    fn help_returns_overview() {
        let mut sys = Sys::new("/tmp".into());
        let resp = sys.execute(Command::Help { topic: None });
        match resp {
            Response::Ok { output } => assert!(output.contains("mux")),
            Response::Error { message } => panic!("Unexpected error: {}", message),
        }
    }

    #[test]
    fn studio_returns_overlay_target() {
        let mut sys = Sys::new("/tmp".into());
        let resp = sys.execute(Command::Studio {
            pane: "%1".into(),
            x: 50,
            y: 30,
        });
        match resp {
            Response::Ok { output } => {
                assert!(output.contains("pane=%1"));
                assert!(output.contains("x=50"));
                assert!(output.contains("y=30"));
            }
            Response::Error { message } => panic!("Unexpected error: {}", message),
        }
    }

    #[test]
    fn setup_hook_returns_bind_command() {
        let mut sys = Sys::new("/tmp".into());
        let resp = sys.execute(Command::SetupHook);
        match resp {
            Response::Ok { output } => {
                assert!(output.contains("tmux bind -n MouseDown3Pane"));
                assert!(output.contains("mux studio"));
            }
            Response::Error { message } => panic!("Unexpected error: {}", message),
        }
    }

    #[test]
    fn remove_hook_returns_unbind_command() {
        let mut sys = Sys::new("/tmp".into());
        let resp = sys.execute(Command::RemoveHook);
        match resp {
            Response::Ok { output } => {
                assert!(output.contains("tmux unbind -n MouseDown3Pane"));
            }
            Response::Error { message } => panic!("Unexpected error: {}", message),
        }
    }
}
