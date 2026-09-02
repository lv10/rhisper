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
use std::path::Path;
#[cfg(target_os = "linux")]
use std::path::PathBuf;
#[cfg(target_os = "linux")]
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use clap::Parser;

use rhisper_core::config::{self, Config, PasteMode};
use rhisper_core::input::{Injector, ModifierKey};
#[cfg(target_os = "linux")]
use rhisper_core::paste;
use rhisper_core::placeholder::{self, Mode as PlaceholderMode};
use rhisper_core::provider::{self, ProviderError, TranscriptionRequest};
use rhisper_core::{audio, clipboard, silence};

const LOGFILE: &str = "/tmp/rhisper.log";
#[cfg(target_os = "linux")]
const DAEMON_LAYOUT_FILE: &str = "/tmp/rhispertoold.layout";
#[cfg(target_os = "linux")]
const DAEMON_LOG_FILE: &str = "/tmp/rhispertoold.log";

#[derive(Parser, Debug)]
#[command(
    name = "rhisper",
    version,
    about = "Dictation at cursor for Linux and macOS"
)]
struct Args {
    /// Use rhispertool/rhispertoold from this binary's directory instead of $PATH (Linux only)
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
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

    let config = match config::load_or_bootstrap() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error: failed to load config: {e}");
            return std::process::ExitCode::FAILURE;
        }
    };

    let mut injector = match build_injector(&config, &args) {
        Ok(i) => i,
        Err(msg) => {
            eprintln!("{msg}");
            return std::process::ExitCode::FAILURE;
        }
    };

    run_toggle(&mut *injector, &config, wrap_key);
    std::process::ExitCode::SUCCESS
}

/// Connects to (starting if necessary) the uinput daemon and wraps it as an
/// Injector. Linux only - see build_injector's macOS counterpart below,
/// which needs no daemon at all.
#[cfg(target_os = "linux")]
fn build_injector(config: &Config, args: &Args) -> Result<Box<dyn Injector>, String> {
    use rhisper_core::input::linux::LinuxInjector;
    use rhisper_core::ipc::ToolClient;

    let rhispertool = tool_path("rhispertool", args.local);
    let rhispertoold = tool_path("rhispertoold", args.local);

    if !binary_exists(&rhispertool) {
        return Err(concat!(
            "Error: rhispertool not found\n",
            "Please either:\n",
            "  - Install the rhisper package for your distro\n",
            "  - Run 'rhisper --local' from the build directory"
        )
        .to_string());
    }

    ensure_daemon_running(&config.keyboard_layout, &rhispertoold).map_err(|e| {
        format!(
            "Error: Failed to start rhispertoold daemon: {e}\nCheck {DAEMON_LOG_FILE} for details"
        )
    })?;

    let tool =
        ToolClient::connect().map_err(|e| format!("failed to connect to rhispertoold: {e}"))?;
    Ok(Box::new(LinuxInjector(tool)))
}

/// Posts CGEvents directly, in-process - no daemon needed, since CGEventPost
/// has no persistent-device registration cost to amortize.
#[cfg(target_os = "macos")]
fn build_injector(_config: &Config, _args: &Args) -> Result<Box<dyn Injector>, String> {
    use macos_accessibility_client::accessibility;
    use rhisper_core::input::macos::CgInjector;

    if !accessibility::application_is_trusted() {
        eprintln!("Warning: rhisper does not have Accessibility permission yet.");
        eprintln!(
            "Run 'rhisper --setup' or grant it in System Settings > Privacy & Security > Accessibility."
        );
    }

    CgInjector::new().map(|i| Box::new(i) as Box<dyn Injector>)
}

/// Shows the "recording"/"transcribing"/"no sound" status either by typing
/// it into the target field or as a desktop notification, per `placeholders`.
///
/// Inline placeholders are removed by counting characters back out again;
/// notifications are removed by closing them. The two are kept behind one
/// type so run_toggle never has to branch on the mode itself.
struct Placeholder<'a> {
    mode: PlaceholderMode,
    config: &'a Config,
    wrap_key: Option<ModifierKey>,
}

impl<'a> Placeholder<'a> {
    fn new(config: &'a Config, wrap_key: Option<ModifierKey>) -> Self {
        let mode = placeholder::resolve(
            config.placeholders,
            config.paste_mode,
            placeholder::notifications_available(),
        );
        Placeholder {
            mode,
            config,
            wrap_key,
        }
    }

    /// Shows a status that a later step replaces or clears.
    fn show(&self, injector: &mut dyn Injector, message: &str) {
        match self.mode {
            PlaceholderMode::Inline => paste(injector, self.config, self.wrap_key, message),
            PlaceholderMode::Notify => placeholder::notify(message, true),
            PlaceholderMode::Off => {}
        }
    }

    /// Removes a status previously passed to show().
    fn clear(&self, injector: &mut dyn Injector, message: &str) {
        match self.mode {
            PlaceholderMode::Inline => delete_n_chars(injector, message.chars().count()),
            PlaceholderMode::Notify => placeholder::dismiss(),
            PlaceholderMode::Off => {}
        }
    }

    /// Shows a final message that nothing else will clear - it lingers just
    /// long enough to be read and then goes away on its own.
    fn show_final(&self, injector: &mut dyn Injector, message: &str) {
        match self.mode {
            PlaceholderMode::Inline => {
                paste(injector, self.config, self.wrap_key, message);
                sleep_secs(0.6);
                delete_n_chars(injector, message.chars().count());
            }
            PlaceholderMode::Notify => placeholder::notify(message, false),
            PlaceholderMode::Off => {}
        }
    }
}

fn run_toggle(injector: &mut dyn Injector, config: &Config, wrap_key: Option<ModifierKey>) {
    let status = Placeholder::new(config, wrap_key);

    if let Some(pid) = audio::running_pid() {
        audio::stop_recording(pid);
        sleep_secs(0.2); // buffer for flush
        status.clear(injector, &config.placeholder_recording);

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
            status.show_final(injector, &config.placeholder_silent);
            audio::remove_recording();
            return;
        }

        status.show(injector, &config.placeholder_transcribing);
        let started = Instant::now();
        let text_result = transcribe(config, audio::RECORDING_PATH);
        status.clear(injector, &config.placeholder_transcribing);

        match text_result {
            Ok(text) => {
                log_event("Transcription", &text, started.elapsed());
                paste(injector, config, wrap_key, &text);
            }
            Err(detail) => {
                log_event(
                    "Transcription",
                    &format!("ERROR: {detail}"),
                    started.elapsed(),
                );
                status.show_final(injector, "(transcription failed - see rhisper --log)");
            }
        }

        audio::remove_recording();
    } else {
        sleep_secs(0.2);
        status.show(injector, &config.placeholder_recording);
        match audio::start_recording() {
            Ok(mut child) => {
                // Blocks until a later "stop" invocation kills the recorder.
                let _ = child.wait();
            }
            Err(e) => {
                eprintln!("Error: failed to start recording: {e}");
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

/// Dispatches on paste-mode. On Linux, Type mode types ASCII chars directly
/// (layout-sensitive) and batches non-ASCII runs through the clipboard,
/// restoring the user's prior clipboard content afterward. On macOS, Type
/// mode types the whole string in one shot via Unicode injection (see
/// Injector::type_text) and never touches the clipboard at all.
fn paste(injector: &mut dyn Injector, config: &Config, wrap_key: Option<ModifierKey>, text: &str) {
    match config.paste_mode {
        PasteMode::Clipboard | PasteMode::ClipboardRestore => {
            let restore = config.paste_mode == PasteMode::ClipboardRestore;
            let old = restore.then(clipboard::get_text);

            clipboard::set_text(text.to_string());
            sleep_secs(config.non_ascii_initial_delay);
            injector.paste_shortcut();

            if let Some(old) = old {
                sleep_secs(config.non_ascii_initial_delay);
                clipboard::set_text(old);
            }
        }
        #[cfg(target_os = "macos")]
        PasteMode::Type => {
            if let Some(key) = wrap_key {
                injector.press_modifier(key);
            }
            injector.type_text(text);
            if let Some(key) = wrap_key {
                injector.press_modifier(key);
            }
        }
        #[cfg(target_os = "linux")]
        PasteMode::Type => {
            let saved_clipboard = clipboard::get_text();

            if let Some(key) = wrap_key {
                injector.press_modifier(key);
            }

            let mut clipboard_modified = false;
            let mut first_chunk = true;
            for chunk in paste::chunk_for_typing(text) {
                match chunk {
                    paste::Chunk::Ascii(c) => {
                        injector.type_ascii_char(c);
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
                        injector.paste_shortcut();
                    }
                }
            }

            if let Some(key) = wrap_key {
                injector.press_modifier(key);
            }

            if clipboard_modified {
                clipboard::set_text(saved_clipboard);
            }
        }
    }
}

fn delete_n_chars(injector: &mut dyn Injector, n: usize) {
    for _ in 0..n {
        injector.backspace();
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

fn resolve_wrap_key(args: &Args) -> Result<Option<ModifierKey>, String> {
    let flags = [
        (args.leftalt, ModifierKey::LeftAlt),
        (args.rightalt, ModifierKey::RightAlt),
        (args.leftctrl, ModifierKey::LeftCtrl),
        (args.rightctrl, ModifierKey::RightCtrl),
        (args.leftshift, ModifierKey::LeftShift),
        (args.rightshift, ModifierKey::RightShift),
        (args.super_key, ModifierKey::Super),
    ];

    let set: Vec<ModifierKey> = flags
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

#[cfg(target_os = "linux")]
fn tool_dir(local: bool) -> Option<PathBuf> {
    if !local {
        return None;
    }
    env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
}

#[cfg(target_os = "linux")]
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
#[cfg(target_os = "linux")]
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

#[cfg(target_os = "linux")]
fn daemon_alive() -> bool {
    Command::new("pgrep")
        .args(["-x", "rhispertoold"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Interactive first-time setup: creates a default config if missing,
/// checks platform permissions (uinput access on Linux, Accessibility on
/// macOS) and runtime dependencies, and optionally prompts for a Groq API
/// key. Replaces configure.sh; unlike it, ordinary
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

    check_platform_permissions();

    let missing_deps: Vec<&str> = REQUIRED_DEPS
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

#[cfg(target_os = "linux")]
const REQUIRED_DEPS: [&str; 3] = ["pw-record", "ffprobe", "ffmpeg"];
#[cfg(target_os = "macos")]
const REQUIRED_DEPS: [&str; 3] = ["rec", "ffprobe", "ffmpeg"];

#[cfg(target_os = "linux")]
fn check_platform_permissions() {
    let uinput = Path::new("/dev/uinput");
    if !uinput.exists() {
        println!("Warning: /dev/uinput not found. Try: sudo modprobe uinput");
    } else if OpenOptions::new().write(true).open(uinput).is_err() {
        println!("Warning: /dev/uinput is not writable by your user.");
        println!("Run: sudo usermod -aG input $USER   (then log out and back in)");
    } else {
        println!("/dev/uinput is writable.");
    }
}

#[cfg(target_os = "macos")]
fn check_platform_permissions() {
    use macos_accessibility_client::accessibility;

    if accessibility::application_is_trusted() {
        println!("Accessibility permission already granted.");
    } else {
        println!("rhisper needs Accessibility permission to type at your cursor.");
        println!("Requesting it now - approve the system prompt, or grant it later in");
        println!("System Settings > Privacy & Security > Accessibility.");
        accessibility::application_is_trusted_with_prompt();
    }
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
