//! MuxUX CLI — the command-line entry point for the Structure App.

use std::path::PathBuf;
use std::process;

use muxux_core::command::Command;
use cmx_utils::response::Response;


fn main() {
    let args: Vec<String> = std::env::args().collect();
    let arg_refs: Vec<&str> = args[1..].iter().map(|s| s.as_str()).collect();

    let cmd = match parse_args(&arg_refs) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("mux: {}", e);
            process::exit(1);
        }
    };

    let _config_dir = resolve_config_dir();

    // For now, execute locally (daemon mode can be added later)
    let mut sys = muxux_core::sys::Sys::new(
        std::env::var("MUX_PROJECT_ROOT").unwrap_or_default(),
    );
    let response = sys.execute(cmd);

    match response {
        Response::Ok { output } => {
            if !output.is_empty() {
                println!("{}", output);
            }
        }
        Response::Error { message } => {
            eprintln!("mux error: {}", message);
            process::exit(1);
        }
    }
}


fn resolve_config_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("MUX_CONFIG_DIR") {
        return PathBuf::from(dir);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".config").join("muxux")
}


fn parse_args(args: &[&str]) -> Result<Command, String> {
    if args.is_empty() {
        return Err("No command specified. Run 'mux help' for usage.".into());
    }

    match args[0] {
        "status" => Ok(Command::Status {
            format: args.get(1).and_then(|a| {
                if *a == "--json" { Some("json".into()) } else { None }
            }),
        }),
        "view" => {
            if args.len() < 2 {
                return Err("Usage: mux view <name>".into());
            }
            Ok(Command::View { name: args[1].into() })
        }
        "help" => Ok(Command::Help {
            topic: args.get(1).map(|s| s.to_string()),
        }),
        "layout" => parse_layout(args),
        "client" => parse_client(args),
        "daemon" => parse_daemon(args),
        "studio" => parse_studio(args),
        "setup" => parse_setup(args),
        "watch" => Ok(Command::Watch {
            since: None,
            timeout: None,
        }),
        _ => Err(format!("Unknown command: '{}'. Run 'mux help' for usage.", args[0])),
    }
}


fn parse_layout(args: &[&str]) -> Result<Command, String> {
    if args.len() < 2 {
        return Err("Usage: mux layout <subcommand> ...".into());
    }
    match args[1] {
        "row" => {
            if args.len() < 3 {
                return Err("Usage: mux layout row <session> [--percent <n>]".into());
            }
            let percent = find_flag(args, "--percent");
            Ok(Command::LayoutRow {
                session: args[2].into(),
                percent,
            })
        }
        "column" => {
            if args.len() < 3 {
                return Err("Usage: mux layout column <session> [--percent <n>]".into());
            }
            let percent = find_flag(args, "--percent");
            Ok(Command::LayoutColumn {
                session: args[2].into(),
                percent,
            })
        }
        "merge" => {
            if args.len() < 3 {
                return Err("Usage: mux layout merge <session>".into());
            }
            Ok(Command::LayoutMerge {
                session: args[2].into(),
            })
        }
        "place" => {
            if args.len() < 4 {
                return Err("Usage: mux layout place <pane> <agent>".into());
            }
            Ok(Command::LayoutPlace {
                pane: args[2].into(),
                agent: args[3].into(),
            })
        }
        "capture" => {
            if args.len() < 3 {
                return Err("Usage: mux layout capture <session>".into());
            }
            Ok(Command::LayoutCapture {
                session: args[2].into(),
            })
        }
        "session" => {
            if args.len() < 3 {
                return Err("Usage: mux layout session <name> [--cwd <path>]".into());
            }
            let cwd = find_flag(args, "--cwd");
            Ok(Command::LayoutSession {
                name: args[2].into(),
                cwd,
            })
        }
        _ => Err(format!("Unknown layout subcommand: '{}'", args[1])),
    }
}


fn parse_client(args: &[&str]) -> Result<Command, String> {
    if args.len() < 2 {
        return Err("Usage: mux client <next|prev>".into());
    }
    match args[1] {
        "next" => Ok(Command::ClientNext),
        "prev" => Ok(Command::ClientPrev),
        _ => Err(format!("Unknown client subcommand: '{}'", args[1])),
    }
}


fn parse_daemon(args: &[&str]) -> Result<Command, String> {
    if args.len() < 2 {
        return Err("Usage: mux daemon <run|stop>".into());
    }
    match args[1] {
        "run" => Ok(Command::DaemonRun),
        "stop" => Ok(Command::DaemonStop),
        _ => Err(format!("Unknown daemon subcommand: '{}'", args[1])),
    }
}


fn parse_studio(args: &[&str]) -> Result<Command, String> {
    let pane = find_flag(args, "--pane")
        .ok_or("Usage: mux studio --pane <id> --x <n> --y <n>")?;
    let x = find_flag(args, "--x")
        .ok_or("Usage: mux studio --pane <id> --x <n> --y <n>")?
        .parse::<u32>()
        .map_err(|_| "Invalid value for --x: expected integer".to_string())?;
    let y = find_flag(args, "--y")
        .ok_or("Usage: mux studio --pane <id> --x <n> --y <n>")?
        .parse::<u32>()
        .map_err(|_| "Invalid value for --y: expected integer".to_string())?;
    Ok(Command::Studio { pane, x, y })
}


fn parse_setup(args: &[&str]) -> Result<Command, String> {
    if args.len() < 2 {
        return Err("Usage: mux setup <hook|unhook>".into());
    }
    match args[1] {
        "hook" => Ok(Command::SetupHook),
        "unhook" => Ok(Command::RemoveHook),
        _ => Err(format!("Unknown setup subcommand: '{}'", args[1])),
    }
}


fn find_flag(args: &[&str], flag: &str) -> Option<String> {
    for (i, arg) in args.iter().enumerate() {
        if *arg == flag {
            return args.get(i + 1).map(|s| s.to_string());
        }
    }
    None
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_studio_all_flags() {
        let args = vec!["studio", "--pane", "%1", "--x", "50", "--y", "30"];
        let cmd = parse_args(&args).unwrap();
        assert_eq!(
            cmd,
            Command::Studio {
                pane: "%1".into(),
                x: 50,
                y: 30,
            }
        );
    }

    #[test]
    fn parse_studio_missing_pane() {
        let args = vec!["studio", "--x", "50", "--y", "30"];
        assert!(parse_args(&args).is_err());
    }

    #[test]
    fn parse_studio_invalid_x() {
        let args = vec!["studio", "--pane", "%1", "--x", "abc", "--y", "30"];
        assert!(parse_args(&args).is_err());
    }

    #[test]
    fn parse_setup_hook() {
        let args = vec!["setup", "hook"];
        let cmd = parse_args(&args).unwrap();
        assert_eq!(cmd, Command::SetupHook);
    }

    #[test]
    fn parse_setup_unhook() {
        let args = vec!["setup", "unhook"];
        let cmd = parse_args(&args).unwrap();
        assert_eq!(cmd, Command::RemoveHook);
    }

    #[test]
    fn parse_setup_no_subcommand() {
        let args = vec!["setup"];
        assert!(parse_args(&args).is_err());
    }

    #[test]
    fn parse_setup_unknown_subcommand() {
        let args = vec!["setup", "foo"];
        assert!(parse_args(&args).is_err());
    }
}
