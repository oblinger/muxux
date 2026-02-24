use crate::command::Command;
use crate::infrastructure::tmux::TmuxCommandBuilder;
use cmx_utils::response::{Action, Direction, Response};


/// Central runtime for MuxUX. Dispatches layout, session, and structure commands.
pub struct Sys {
    project_root: String,
    actions: Vec<Action>,
}


impl Sys {
    pub fn new(project_root: String) -> Sys {
        Sys {
            project_root,
            actions: Vec::new(),
        }
    }

    /// The single dispatch method.
    pub fn execute(&mut self, cmd: Command) -> Response {
        self.actions.clear();
        match cmd {
            Command::Status { format } => self.cmd_status(format),
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
}
