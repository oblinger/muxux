//! MuxUX socket client — sends commands to the MuxUX daemon.

use std::path::Path;

use muxux_core::command::Command;
use cmx_utils::response::Response;


/// Send a command to the MuxUX daemon via Unix socket.
pub fn send_command(config_dir: &Path, cmd: &Command, timeout_ms: u64) -> Result<Response, String> {
    let sock_path = config_dir.join("mux.sock");
    cmx_utils::client::send_and_receive(&sock_path, cmd, timeout_ms)
}
