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

/// One capture source offered by the audio server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureSource {
    /// The identifier `audio-device` expects.
    pub name: String,
    /// Human-readable label, for listings.
    pub description: String,
    pub is_default: bool,
    /// Numeric aliases `pw-record --target` also accepts.
    pub aliases: Vec<String>,
}

#[cfg(target_os = "linux")]
impl CaptureSource {
    fn matches(&self, device: &str) -> bool {
        self.name == device || self.aliases.iter().any(|a| a == device)
    }
}

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

/// Lists the capture sources the recorder can be pointed at.
///
/// Read from `pw-dump` rather than `pw-record --list-targets`, which only
/// exists in recent PipeWire builds; the dump has been stable far longer
/// and additionally reveals which source is the current default.
#[cfg(target_os = "linux")]
pub fn capture_sources() -> Result<Vec<CaptureSource>, String> {
    let output = Command::new("pw-dump")
        .stderr(Stdio::null())
        .output()
        .map_err(|e| format!("failed to run pw-dump: {e}"))?;

    if !output.status.success() {
        return Err("pw-dump failed - is PipeWire running?".to_string());
    }

    parse_capture_sources(&output.stdout)
}

#[cfg(target_os = "linux")]
fn parse_capture_sources(dump: &[u8]) -> Result<Vec<CaptureSource>, String> {
    let objects: Vec<serde_json::Value> =
        serde_json::from_slice(dump).map_err(|e| format!("failed to parse pw-dump output: {e}"))?;

    let default_source = objects
        .iter()
        .filter(|o| o["props"]["metadata.name"] == "default")
        .flat_map(|o| o["metadata"].as_array().into_iter().flatten())
        .find(|m| m["key"] == "default.audio.source")
        .and_then(|m| m["value"]["name"].as_str())
        .unwrap_or_default()
        .to_string();

    Ok(objects
        .iter()
        .filter(|o| o["type"] == "PipeWire:Interface:Node")
        .filter(|o| o["info"]["props"]["media.class"] == "Audio/Source")
        .filter_map(|o| {
            let props = &o["info"]["props"];
            let name = props["node.name"].as_str()?.to_string();
            let description = props["node.description"]
                .as_str()
                .unwrap_or(&name)
                .to_string();
            let aliases = [o["id"].as_i64(), props["object.serial"].as_i64()]
                .into_iter()
                .flatten()
                .map(|n| n.to_string())
                .collect();

            Some(CaptureSource {
                is_default: name == default_source,
                name,
                description,
                aliases,
            })
        })
        .collect())
}

/// What could be learned about the configured capture device.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeviceInfo {
    /// `None` when presence could not be determined - the audio server did
    /// not respond, or the platform cannot enumerate sources.
    pub available: Option<bool>,
    /// Human-readable name of the source that will actually be recorded
    /// from, e.g. "Anua Mic CM 900 Mono". `None` when unknown.
    pub description: Option<String>,
}

/// Looks up presence and human-readable name in one go.
///
/// An empty `device` means the system default, whose description is worth
/// resolving too: it is the answer to "which microphone is this recording
/// from" that the user actually wants to see.
#[cfg(target_os = "linux")]
pub fn inspect_device(device: &str) -> DeviceInfo {
    let Ok(sources) = capture_sources() else {
        return DeviceInfo::default();
    };

    let source = if device.is_empty() {
        sources.iter().find(|s| s.is_default)
    } else {
        sources.iter().find(|s| s.matches(device))
    };

    DeviceInfo {
        // An empty device is always "available" - there is nothing to miss.
        available: Some(device.is_empty() || source.is_some()),
        description: source.map(|s| s.description.clone()),
    }
}

#[cfg(target_os = "macos")]
pub fn inspect_device(_device: &str) -> DeviceInfo {
    DeviceInfo::default()
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

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    const DUMP: &str = r#"[
      {"type":"PipeWire:Interface:Metadata","props":{"metadata.name":"default"},
       "metadata":[{"key":"default.audio.source","value":{"name":"bluez_input.AA"}}]},
      {"id":62,"type":"PipeWire:Interface:Node","info":{"props":{
        "media.class":"Audio/Source","node.name":"alsa_input.builtin",
        "node.description":"Built-in Analog Stereo","object.serial":301}}},
      {"id":147,"type":"PipeWire:Interface:Node","info":{"props":{
        "media.class":"Audio/Source","node.name":"bluez_input.AA",
        "node.description":"Headset","object.serial":412}}},
      {"id":61,"type":"PipeWire:Interface:Node","info":{"props":{
        "media.class":"Audio/Sink","node.name":"alsa_output.builtin",
        "node.description":"Speakers","object.serial":300}}}
    ]"#;

    #[test]
    fn parses_sources_and_marks_the_default() {
        let sources = parse_capture_sources(DUMP.as_bytes()).unwrap();
        let names: Vec<&str> = sources.iter().map(|s| s.name.as_str()).collect();
        // The sink is not a capture source and must not be offered.
        assert_eq!(names, ["alsa_input.builtin", "bluez_input.AA"]);
        assert_eq!(sources[0].description, "Built-in Analog Stereo");
        assert!(!sources[0].is_default);
        assert!(sources[1].is_default);
    }

    #[test]
    fn sources_match_by_name_or_numeric_alias() {
        let sources = parse_capture_sources(DUMP.as_bytes()).unwrap();
        let builtin = &sources[0];
        assert!(builtin.matches("alsa_input.builtin"));
        assert!(builtin.matches("62"), "node id is a valid --target");
        assert!(builtin.matches("301"), "object.serial is a valid --target");
        assert!(!builtin.matches("bluez_input.AA"));
        assert!(
            !builtin.matches("412"),
            "another node's serial must not match"
        );
    }

    #[test]
    fn malformed_dump_is_an_error_not_an_empty_list() {
        assert!(parse_capture_sources(b"not json").is_err());
    }
}
