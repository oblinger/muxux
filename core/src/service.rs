//! MuxUX service — Unix socket listener wrapping cmx-utils service.

use std::os::unix::net::UnixStream;
use std::path::Path;

use cmx_utils::watch::WatchRegistry;

use crate::command::Command;
use crate::sys::Sys;


/// Handle a single connection: read command, dispatch through Sys.
pub fn handle_connection(
    mut stream: UnixStream,
    sys: &mut Sys,
    registry: &mut WatchRegistry,
) -> Result<bool, String> {
    let raw = cmx_utils::service::read_frame(&mut stream)?;

    let cmd: Command = serde_json::from_slice(&raw)
        .map_err(|e| format!("Failed to parse command JSON: {}", e))?;

    match cmd {
        Command::Watch { since, timeout } => {
            let since_ms = since.and_then(|s| s.parse::<u64>().ok());
            let timeout_ms = timeout
                .and_then(|t| t.parse::<u64>().ok())
                .unwrap_or(30_000);
            registry.register(stream, since_ms, timeout_ms);
            Ok(false)
        }
        Command::DaemonStop => {
            let response = sys.execute(cmd);
            cmx_utils::service::write_response(&mut stream, &response)?;
            Ok(true) // signal shutdown
        }
        _ => {
            let summary = format!("{:?}", cmd);
            let summary = if summary.len() > 200 {
                format!("{}...", &summary[..200])
            } else {
                summary
            };
            let response = sys.execute(cmd);
            cmx_utils::service::write_response(&mut stream, &response)?;

            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            registry.notify_all(&summary, now_ms);

            Ok(false)
        }
    }
}


/// Start the MuxUX service socket.
pub fn start(config_dir: &Path) -> Result<cmx_utils::service::ServiceSocket, String> {
    cmx_utils::service::ServiceSocket::start(config_dir, "mux")
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_returns_socket_name() {
        // Verify the socket would be named mux.sock
        let dir = std::env::temp_dir().join(format!("muxux-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let sock = start(&dir).unwrap();
        assert!(sock.path().to_str().unwrap().contains("mux.sock"));
        sock.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
