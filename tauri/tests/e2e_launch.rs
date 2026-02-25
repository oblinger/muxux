//! End-to-end launch test for MuxUX.
//!
//! Launches the compiled binary and verifies it starts without errors.
//! Only runs when the `e2e` feature is enabled:
//!
//!     cargo test -p muxux --features e2e

#![cfg(feature = "e2e")]

use std::process::Command;
use std::time::Duration;

/// Launch the muxux binary briefly and verify no errors on stderr.
///
/// The binary is expected to open a Tauri window. We let it run for a
/// couple of seconds, then kill it. Stderr must not contain any error
/// messages about missing configuration (e.g., macos-private-api).
#[test]
fn launch_produces_no_errors() {
    // Locate the binary next to the test binary (same target dir)
    let binary = env!("CARGO_BIN_EXE_muxux");

    let mut child = Command::new(binary)
        .env("MUX_PROJECT_ROOT", "/tmp")
        .stderr(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("failed to launch muxux binary");

    // Let it run briefly so Tauri initialization completes
    std::thread::sleep(Duration::from_secs(3));

    // Kill the process (it's a GUI app, won't exit on its own)
    let _ = child.kill();
    let output = child.wait_with_output().expect("failed to read output");

    let stderr = String::from_utf8_lossy(&output.stderr);

    // The specific error we're guarding against
    assert!(
        !stderr.contains("macos-private-api"),
        "Binary emitted macos-private-api error on stderr:\n{}",
        stderr,
    );

    // Catch any other Tauri configuration warnings
    assert!(
        !stderr.contains("is not enabled"),
        "Binary emitted a 'not enabled' warning on stderr:\n{}",
        stderr,
    );
}
