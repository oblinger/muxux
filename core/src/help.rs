//! Help system for MuxUX commands.

pub fn help_text(topic: Option<&str>) -> String {
    match topic {
        None => overview(),
        Some(t) => {
            if let Some(text) = command_help(t) {
                return text;
            }
            if let Some(text) = group_help(t) {
                return text;
            }
            format!("Unknown help topic: '{}'. Run 'mux help' for a list of commands.", t)
        }
    }
}


fn overview() -> String {
    "\
mux — MuxUX command-line interface (Structure App)

Usage: mux <command> [args...]

Commands:
  status [--json]             Show MuxUX status
  view <name>                Look up a session or layout by name
  help [topic]               Show help

Layout commands:
  layout row <session> [--percent <n>]     Split session horizontally
  layout column <session> [--percent <n>]  Split session vertically
  layout merge <session>                   Merge all panes into one
  layout place <pane> <agent>              Place an agent in a pane
  layout capture <session>                 Capture pane contents
  layout session <name> [--cwd <path>]     Create a new tmux session

Client commands:
  client next                Switch to next client view
  client prev                Switch to previous client view

Watch command:
  watch [--since <ms>] [--timeout <ms>]  Stream state changes

Daemon commands:
  daemon run                 Start MuxUX daemon in foreground
  daemon stop                Stop running daemon

Run 'mux help <command>' for detailed help on a specific command."
        .into()
}


fn group_help(group: &str) -> Option<String> {
    let text = match group {
        "layout" => "\
Layout commands — manage tmux sessions and pane layout

  layout row <session> [--percent <n>]
    Split the session with a horizontal divider. Default 50%.

  layout column <session> [--percent <n>]
    Split the session with a vertical divider. Default 50%.

  layout merge <session>
    Merge all panes in a session into a single pane.

  layout place <pane> <agent>
    Place an agent into a specific tmux pane (e.g. %3).

  layout capture <session>
    Capture the current content of all panes in a session.

  layout session <name> [--cwd <path>]
    Create a new tmux session. Uses project_root as default cwd.",

        "client" => "\
Client commands — navigate between client views

  client next
    Switch to the next client view.

  client prev
    Switch to the previous client view.",

        "watch" => "\
Watch command — stream state changes

  watch [--since <ms>] [--timeout <ms>]
    Stream state change events to stdout as they occur.",

        "daemon" => "\
Daemon commands — manage the MuxUX daemon process

  daemon run
    Start the MuxUX daemon in the foreground.

  daemon stop
    Stop the running MuxUX daemon gracefully.",

        _ => return None,
    };
    Some(text.into())
}


fn command_help(command: &str) -> Option<String> {
    let text = match command {
        "status" => "mux status — show MuxUX status\n\nUsage: mux status [--json]",
        "view" => "mux view — look up a session or layout\n\nUsage: mux view <name>",
        "help" => "mux help — show help\n\nUsage: mux help [topic]",
        "layout.row" => "mux layout row — horizontal split\n\nUsage: mux layout row <session> [--percent <n>]",
        "layout.column" => "mux layout column — vertical split\n\nUsage: mux layout column <session> [--percent <n>]",
        "layout.merge" => "mux layout merge — merge panes\n\nUsage: mux layout merge <session>",
        "layout.place" => "mux layout place — place agent in pane\n\nUsage: mux layout place <pane> <agent>",
        "layout.capture" => "mux layout capture — capture pane contents\n\nUsage: mux layout capture <session>",
        "layout.session" => "mux layout session — create tmux session\n\nUsage: mux layout session <name> [--cwd <path>]",
        "client.next" => "mux client next — switch to next view\n\nUsage: mux client next",
        "client.prev" => "mux client prev — switch to previous view\n\nUsage: mux client prev",
        "watch" => "mux watch — stream state changes\n\nUsage: mux watch [--since <ms>] [--timeout <ms>]",
        "daemon.run" => "mux daemon run — start daemon\n\nUsage: mux daemon run",
        "daemon.stop" => "mux daemon stop — stop daemon\n\nUsage: mux daemon stop",
        _ => return None,
    };
    Some(text.into())
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overview_contains_layout() {
        let text = help_text(None);
        assert!(text.contains("Layout commands:"));
        assert!(text.contains("Client commands:"));
    }

    #[test]
    fn group_help_layout() {
        let text = help_text(Some("layout"));
        assert!(text.contains("layout row"));
        assert!(text.contains("layout column"));
        assert!(text.contains("layout merge"));
    }

    #[test]
    fn command_help_layout_row() {
        let text = help_text(Some("layout.row"));
        assert!(text.contains("Usage:"));
        assert!(text.contains("--percent"));
    }

    #[test]
    fn unknown_topic() {
        let text = help_text(Some("bogus"));
        assert!(text.contains("Unknown help topic"));
    }
}
