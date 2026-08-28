// audio.rs - pw-record process management for the record/stop toggle.
//
// Recording state is tracked by the pw-record child's PID in a small file
// written when recording starts, rather than pattern-matching the process
// table (which could be confused by an unrelated process that happens to
// mention the same path on its command line). The PID is checked for
// liveness (not just existence) before being trusted.

use std::fs;
use std::io;
use std::process::{Child, Command, Stdio};

pub const RECORDING_PATH: &str = "/tmp/rhisper.wav";
const PID_FILE: &str = "/tmp/rhisper.pid";

/// Returns the PID of rhisper's own in-progress recording, if any. A PID
/// file whose process is no longer alive is treated as stale and removed.
pub fn running_pid() -> Option<i32> {
    let pid: i32 = fs::read_to_string(PID_FILE).ok()?.trim().parse().ok()?;

    // Signal 0 performs no-op existence/permission checks only.
    let alive = unsafe { libc::kill(pid, 0) == 0 };
    if alive {
        Some(pid)
    } else {
        let _ = fs::remove_file(PID_FILE);
        None
    }
}

/// Starts `pw-record` as a foreground child (the caller is expected to
/// `.wait()` on it, exactly mirroring the original script's behavior of
/// blocking the "start" invocation until a later "stop" invocation kills
/// the recording), and persists its PID for that later invocation to find.
pub fn start_recording() -> io::Result<Child> {
    let child = Command::new("pw-record")
        .args(["--channels=1", "--rate=16000", RECORDING_PATH])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    fs::write(PID_FILE, child.id().to_string())?;
    Ok(child)
}

/// Stops an in-progress recording by PID (found via running_pid()).
pub fn stop_recording(pid: i32) {
    unsafe {
        libc::kill(pid, libc::SIGTERM);
    }
    let _ = fs::remove_file(PID_FILE);
}

pub fn remove_recording() {
    let _ = fs::remove_file(RECORDING_PATH);
}
