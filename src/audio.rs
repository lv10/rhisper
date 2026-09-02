// audio.rs - recorder process management for the record/stop toggle.
//
// Recording state is tracked by the recorder child's PID in a small file
// written when recording starts, rather than pattern-matching the process
// table (which could be confused by an unrelated process that happens to
// mention the same path on its command line). The PID is checked for
// liveness (not just existence) before being trusted.

use std::fs;
use std::io;
use std::process::{Child, Command, Stdio};

pub const RECORDING_PATH: &str = "/tmp/rhisper.wav";
const PID_FILE: &str = "/tmp/rhisper.pid";

/// `device` selects an explicit capture source; empty means "whatever the
/// system default is", which is the historical behavior. On Linux it is a
/// PipeWire target - either a node name (`alsa_input.usb-...`) or a numeric
/// node id, both accepted by `pw-record --target`.
#[cfg(target_os = "linux")]
fn record_command(device: &str) -> Command {
    let mut cmd = Command::new("pw-record");
    if !device.is_empty() {
        cmd.arg(format!("--target={device}"));
    }
    cmd.args(["--channels=1", "--rate=16000", RECORDING_PATH]);
    cmd
}

// sox's `rec` grabs the system default input device with no numeric
// device-index guessing, unlike `ffmpeg -f avfoundation` (whose index can
// shift depending on what's plugged in).
// On macOS sox picks its input via the AUDIODEV environment variable, so an
// explicit device is a coreaudio device name rather than a command-line flag.
#[cfg(target_os = "macos")]
fn record_command(device: &str) -> Command {
    let mut cmd = Command::new("rec");
    if !device.is_empty() {
        cmd.env("AUDIODEV", device);
    }
    cmd.args([
        "-q",
        "-c",
        "1",
        "-r",
        "16000",
        "-b",
        "16",
        "-e",
        "signed-integer",
        RECORDING_PATH,
    ]);
    cmd
}

#[cfg(target_os = "linux")]
fn stop_signal() -> i32 {
    libc::SIGTERM
}

// sox finalizes the WAV header on SIGINT, not SIGTERM - unconfirmed on real
// hardware (no Mac available this session), flagged for follow-up.
#[cfg(target_os = "macos")]
fn stop_signal() -> i32 {
    libc::SIGINT
}

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

/// Starts the platform recorder as a foreground child (the caller is
/// expected to `.wait()` on it, exactly mirroring the original script's
/// behavior of blocking the "start" invocation until a later "stop"
/// invocation kills the recording), and persists its PID for that later
/// invocation to find.
pub fn start_recording(device: &str) -> io::Result<Child> {
    let child = record_command(device)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    fs::write(PID_FILE, child.id().to_string())?;
    Ok(child)
}

/// Stops an in-progress recording by PID (found via running_pid()).
pub fn stop_recording(pid: i32) {
    unsafe {
        libc::kill(pid, stop_signal());
    }
    let _ = fs::remove_file(PID_FILE);
}

pub fn remove_recording() {
    let _ = fs::remove_file(RECORDING_PATH);
}
