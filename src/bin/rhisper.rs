// rhisper - dictation at cursor. Orchestrates the record/stop toggle,
// config loading, silence detection, paste dispatch, transcription, and
// the typed-placeholder progress UX.
//
// Runtime dependencies are minimal: JSON parsing, float math, HTTP, and
// clipboard access are all in-process. Only pw-record and ffmpeg/ffprobe
// remain external processes.

use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use clap::Parser;

use rhisper_core::config::{self, Config, PasteMode};
use rhisper_core::ipc::{Command as ToolCommand, ToolClient};
use rhisper_core::provider::{self, ProviderError, TranscriptionRequest};
use rhisper_core::{audio, clipboard, paste, silence};

const LOGFILE: &str = "/tmp/rhisper.log";
const DAEMON_LAYOUT_FILE: &str = "/tmp/rhispertoold.layout";
const DAEMON_LOG_FILE: &str = "/tmp/rhispertoold.log";

#[derive(Parser, Debug)]
#[command(name = "rhisper", version, about = "Dictation at cursor for Linux")]
struct Args {
    /// Use rhispertool/rhispertoold from this binary's directory instead of $PATH
    #[arg(long)]
    local: bool,

    /// Print the log file and exit
    #[arg(long)]
    log: bool,

    /// Print the config file and exit
    #[arg(long)]
    config: bool,

    /// Run interactive first-time setup (API key prompt, dependency checks)
    #[arg(long)]
    setup: bool,

    #[arg(long)]
    leftalt: bool,
    #[arg(long)]
    rightalt: bool,
    #[arg(long)]
    leftctrl: bool,
    #[arg(long)]
    rightctrl: bool,
    #[arg(long)]
    leftshift: bool,
    #[arg(long)]
    rightshift: bool,
    #[arg(long = "super")]
    super_key: bool,
}

fn main() -> std::process::ExitCode {
    load_dotenv();
    let args = Args::parse();

    if args.log {
        return print_file_or_notice(Path::new(LOGFILE));
    }
    if args.config {
        return print_file_or_notice(&config::config_path());
    }
    if args.setup {
        return match run_setup() {
            Ok(()) => std::process::ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("setup failed: {e}");
                std::process::ExitCode::FAILURE
            }
        };
    }

    let wrap_key = match resolve_wrap_key(&args) {
        Ok(k) => k,
        Err(msg) => {
            eprintln!("{msg}");
            return std::process::ExitCode::FAILURE;
        }
    };

    let rhispertool = tool_path("rhispertool", args.local);
    let rhispertoold = tool_path("rhispertoold", args.local);

    if !binary_exists(&rhispertool) {
        eprintln!("Error: rhispertool not found");
        eprintln!("Please either:");
        eprintln!("  - Install the rhisper package for your distro");
        eprintln!("  - Run 'rhisper --local' from the build directory");
        return std::process::ExitCode::FAILURE;
    }

    let config = match config::load_or_bootstrap() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error: failed to load config: {e}");
            return std::process::ExitCode::FAILURE;
        }
    };

    if let Err(e) = ensure_daemon_running(&config.keyboard_layout, &rhispertoold) {
        eprintln!("Error: Failed to start rhispertoold daemon: {e}");
        eprintln!("Check {DAEMON_LOG_FILE} for details");
        return std::process::ExitCode::FAILURE;
    }

    let tool = match ToolClient::connect() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("failed to connect to rhispertoold: {e}");
            return std::process::ExitCode::FAILURE;
        }
    };

    run_toggle(&tool, &config, wrap_key);
    std::process::ExitCode::SUCCESS
}

fn run_toggle(tool: &ToolClient, config: &Config, wrap_key: Option<ToolCommand>) {
    if let Some(pid) = audio::running_pid() {
        audio::stop_recording(pid);
        sleep_secs(0.2); // buffer for flush
        delete_n_chars(tool, "(recording...)".chars().count());

        let report = silence::analyze(
            audio::RECORDING_PATH,
            config.silence_threshold,
            config.min_speech_seconds,
        );
        if report.is_silent {
            log_event(
                "Silence check",
                &format!(
                    "longest active stretch {:.3}s < min-speech-seconds {:.3}s (threshold {}dB)",
                    report.longest_active_seconds,
                    config.min_speech_seconds,
                    config.silence_threshold
                ),
                Duration::ZERO,
            );
            paste(tool, config, wrap_key, "(no sound detected)");
            sleep_secs(0.6);
            delete_n_chars(tool, "(no sound detected)".chars().count());
            audio::remove_recording();
            return;
        }

        paste(tool, config, wrap_key, "(transcribing...)");
        let started = Instant::now();
        let text_result = transcribe(config, audio::RECORDING_PATH);
        delete_n_chars(tool, "(transcribing...)".chars().count());

        match text_result {
            Ok(text) => {
                log_event("Transcription", &text, started.elapsed());
                paste(tool, config, wrap_key, &text);
            }
            Err(detail) => {
                log_event(
                    "Transcription",
                    &format!("ERROR: {detail}"),
                    started.elapsed(),
                );
                let placeholder = "(transcription failed - see rhisper --log)";
                paste(tool, config, wrap_key, placeholder);
                sleep_secs(0.6);
                delete_n_chars(tool, placeholder.chars().count());
            }
        }

        audio::remove_recording();
    } else {
        sleep_secs(0.2);
        paste(tool, config, wrap_key, "(recording...)");
        match audio::start_recording() {
            Ok(mut child) => {
                // Blocks until a later "stop" invocation kills pw-record.
                let _ = child.wait();
            }
            Err(e) => {
                eprintln!("Error: failed to start pw-record: {e}");
            }
        }
    }
}

fn transcribe(config: &Config, recording: &str) -> Result<String, String> {
    let duration = silence::get_duration(recording);
    let (provider, api_key_env) = provider::resolve_provider(config);

    let model = if config.model.is_empty() {
        provider::default_model(config.provider, duration, config.long_recording_threshold)
            .to_string()
    } else {
        config.model.clone()
    };

    let language = (!config.language.is_empty()).then_some(config.language.as_str());

    let request = TranscriptionRequest {
        audio_path: Path::new(recording),
        model: &model,
        prompt: &config.transcription_prompt,
        language,
    };

    provider.transcribe(&request).map_err(|e| match e {
        ProviderError::MissingApiKey => format!("missing API key: set ${api_key_env}"),
        other => other.to_string(),
    })
}

/// Dispatches on paste-mode, typing ASCII chars directly (layout-sensitive)
/// and batching non-ASCII runs through the clipboard, restoring the user's
/// prior clipboard content afterward.
fn paste(tool: &ToolClient, config: &Config, wrap_key: Option<ToolCommand>, text: &str) {
    match config.paste_mode {
        PasteMode::Clipboard | PasteMode::ClipboardRestore => {
            let restore = config.paste_mode == PasteMode::ClipboardRestore;
            let old = restore.then(clipboard::get_text);

            clipboard::set_text(text.to_string());
            sleep_secs(config.non_ascii_initial_delay);
            let _ = tool.paste();

            if let Some(old) = old {
                sleep_secs(config.non_ascii_initial_delay);
                clipboard::set_text(old);
            }
        }
        PasteMode::Type => {
            let saved_clipboard = clipboard::get_text();

            if let Some(key) = wrap_key {
                let _ = tool.press(key);
            }

            let mut clipboard_modified = false;
            let mut first_chunk = true;
            for chunk in paste::chunk_for_typing(text) {
                match chunk {
                    paste::Chunk::Ascii(c) => {
                        let _ = tool.type_char(c);
                    }
                    paste::Chunk::NonAscii(s) => {
                        clipboard::set_text(s);
                        clipboard_modified = true;
                        sleep_secs(if first_chunk {
                            config.non_ascii_initial_delay
                        } else {
                            config.non_ascii_default_delay
                        });
                        first_chunk = false;
                        let _ = tool.paste();
                    }
                }
            }

            if let Some(key) = wrap_key {
                let _ = tool.press(key);
            }

            if clipboard_modified {
                clipboard::set_text(saved_clipboard);
            }
        }
    }
}

fn delete_n_chars(tool: &ToolClient, n: usize) {
    for _ in 0..n {
        let _ = tool.backspace();
    }
}

fn sleep_secs(secs: f64) {
    std::thread::sleep(Duration::from_secs_f64(secs.max(0.0)));
}

fn log_event(title: &str, result: &str, elapsed: Duration) {
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(LOGFILE) {
        let _ = writeln!(f, "=== {title} ===");
        let _ = writeln!(f, "Result: [{result}]");
        let _ = writeln!(f, "Time: {:.3}s", elapsed.as_secs_f64());
    }
}

fn print_file_or_notice(path: &Path) -> std::process::ExitCode {
    match fs::read_to_string(path) {
        Ok(contents) => print!("{contents}"),
        Err(_) => eprintln!("No file found at {}", path.display()),
    }
    std::process::ExitCode::SUCCESS
}

fn resolve_wrap_key(args: &Args) -> Result<Option<ToolCommand>, String> {
    let flags = [
        (args.leftalt, ToolCommand::LeftAlt),
        (args.rightalt, ToolCommand::RightAlt),
        (args.leftctrl, ToolCommand::LeftCtrl),
        (args.rightctrl, ToolCommand::RightCtrl),
        (args.leftshift, ToolCommand::LeftShift),
        (args.rightshift, ToolCommand::RightShift),
        (args.super_key, ToolCommand::Super),
    ];

    let set: Vec<ToolCommand> = flags
        .iter()
        .filter(|(on, _)| *on)
        .map(|(_, c)| *c)
        .collect();
    match set.len() {
        0 => Ok(None),
        1 => Ok(Some(set[0])),
        _ => Err("Error: Multiple wrap keys not yet supported".to_string()),
    }
}

fn tool_dir(local: bool) -> Option<PathBuf> {
    if !local {
        return None;
    }
    env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
}

fn tool_path(name: &str, local: bool) -> String {
    match tool_dir(local) {
        Some(dir) => dir.join(name).to_string_lossy().into_owned(),
        None => name.to_string(),
    }
}

fn binary_exists(path_or_name: &str) -> bool {
    if path_or_name.contains('/') {
        return Path::new(path_or_name).is_file();
    }
    env::var("PATH")
        .map(|path_var| {
            path_var
                .split(':')
                .any(|dir| Path::new(dir).join(path_or_name).is_file())
        })
        .unwrap_or(false)
}

/// Auto-starts the daemon if it isn't running, and restarts it if its
/// persisted keyboard layout doesn't match the current config (the daemon
/// only reads RHISPER_LAYOUT once, at startup).
fn ensure_daemon_running(layout: &str, rhispertoold: &str) -> io::Result<()> {
    if daemon_alive() {
        let running_layout = fs::read_to_string(DAEMON_LAYOUT_FILE)
            .unwrap_or_default()
            .trim()
            .to_string();
        if running_layout == layout {
            return Ok(());
        }
        let _ = Command::new("pkill").args(["-x", "rhispertoold"]).status();
        std::thread::sleep(Duration::from_millis(300));
    }

    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(DAEMON_LOG_FILE)?;
    Command::new(rhispertoold)
        .env("RHISPER_LAYOUT", layout)
        .stdout(Stdio::null())
        .stderr(Stdio::from(log))
        .spawn()?;

    std::thread::sleep(Duration::from_secs(1));

    if !daemon_alive() {
        return Err(io::Error::other("rhispertoold did not start"));
    }
    Ok(())
}

fn daemon_alive() -> bool {
    Command::new("pgrep")
        .args(["-x", "rhispertoold"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Interactive first-time setup: creates a default config if missing,
/// checks /dev/uinput access and runtime dependencies, and optionally
/// prompts for a Groq API key. Replaces configure.sh; unlike it, ordinary
/// `rhisper` invocations never require this to have been run first (see
/// config::load_or_bootstrap's non-interactive template bootstrap).
fn run_setup() -> io::Result<()> {
    let config_path = config::config_path();
    if !config_path.exists() {
        fs::create_dir_all(config::config_dir())?;
        fs::write(&config_path, config::DEFAULT_RHISPERRC)?;
        println!("Created default config at {}", config_path.display());
    } else {
        println!("Config already exists at {}", config_path.display());
    }

    let uinput = Path::new("/dev/uinput");
    if !uinput.exists() {
        println!("Warning: /dev/uinput not found. Try: sudo modprobe uinput");
    } else if OpenOptions::new().write(true).open(uinput).is_err() {
        println!("Warning: /dev/uinput is not writable by your user.");
        println!("Run: sudo usermod -aG input $USER   (then log out and back in)");
    } else {
        println!("/dev/uinput is writable.");
    }

    let missing_deps: Vec<&str> = ["pw-record", "ffprobe", "ffmpeg"]
        .into_iter()
        .filter(|d| !binary_exists(d))
        .collect();
    if !missing_deps.is_empty() {
        println!("Missing dependencies: {}", missing_deps.join(", "));
    }

    let has_key = env::var("GROQ_API_KEY").is_ok() || env_file_has_groq_key();
    if !has_key {
        print!("Enter your Groq API Key (optional, press Enter to skip): ");
        io::stdout().flush()?;
        let mut key = String::new();
        io::stdin().read_line(&mut key)?;
        let key = key.trim();
        if !key.is_empty() {
            let home = env::var("HOME").unwrap_or_default();
            let mut f = OpenOptions::new()
                .create(true)
                .append(true)
                .open(format!("{home}/.env"))?;
            writeln!(f, "GROQ_API_KEY={key}")?;
        }
    }

    println!("Configuration complete. Run 'rhisper' to start dictating.");
    Ok(())
}

/// Loads `~/.env` (KEY=VALUE per line) into the process environment, for
/// picking up GROQ_API_KEY/OPENAI_API_KEY/RHISPER_API_KEY without requiring
/// them to be exported in the shell. Real shell-exported variables always
/// take precedence over the file.
fn load_dotenv() {
    let home = env::var("HOME").unwrap_or_default();
    let Ok(contents) = fs::read_to_string(format!("{home}/.env")) else {
        return;
    };
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            if env::var(key).is_err() {
                // SAFETY: called once, single-threaded, before any other
                // threads are spawned (clipboard's background thread only
                // starts later, during paste()).
                unsafe { env::set_var(key, value.trim()) };
            }
        }
    }
}

fn env_file_has_groq_key() -> bool {
    let home = env::var("HOME").unwrap_or_default();
    fs::read_to_string(format!("{home}/.env"))
        .map(|contents| {
            contents
                .lines()
                .any(|l| l.trim_start().starts_with("GROQ_API_KEY"))
        })
        .unwrap_or(false)
}
