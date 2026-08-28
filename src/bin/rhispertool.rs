// rhispertool - combined daemon and client for text input via uinput.
// Mode is selected by argv[0] (the "rhispertoold" symlink installed by
// packaging triggers daemon mode) or an explicit `--daemon` flag.
//
// Linux-only: this binary exists because uinput requires a persistent
// virtual device owned by a long-lived daemon (see src/input/mod.rs's
// module comment for why). Other platforms get a stub main() below since
// Cargo has no declarative way to exclude a [[bin]] target per platform.

#[cfg(not(target_os = "linux"))]
fn main() -> std::process::ExitCode {
    eprintln!("rhispertool is Linux-only and is not used on this platform.");
    std::process::ExitCode::FAILURE
}

#[cfg(target_os = "linux")]
use std::env;
#[cfg(target_os = "linux")]
use std::fs;
#[cfg(target_os = "linux")]
use std::io::Write;
#[cfg(target_os = "linux")]
use std::path::Path;
#[cfg(target_os = "linux")]
use std::process::ExitCode;

#[cfg(target_os = "linux")]
use rhisper_core::input::uinput::{self, RhisperDevice};
#[cfg(target_os = "linux")]
use rhisper_core::ipc::{self, Command};

#[cfg(target_os = "linux")]
const LAYOUT_FILE: &str = "/tmp/rhispertoold.layout";

#[cfg(target_os = "linux")]
fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let prog = args
        .first()
        .and_then(|p| {
            Path::new(p)
                .file_name()
                .map(|f| f.to_string_lossy().into_owned())
        })
        .unwrap_or_default();

    let is_daemon = prog == "rhispertoold" || args.get(1).map(String::as_str) == Some("--daemon");

    if is_daemon {
        run_daemon()
    } else {
        run_client(&args)
    }
}

#[cfg(target_os = "linux")]
fn run_daemon() -> ExitCode {
    let mut device = match RhisperDevice::create() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("failed to open /dev/uinput: {e}");
            return ExitCode::FAILURE;
        }
    };

    let socket = match ipc::bind_daemon_socket() {
        Ok(s) => s,
        Err(e) => {
            if e.kind() == std::io::ErrorKind::AddrInUse {
                eprintln!("rhispertoold is already running");
            } else {
                eprintln!("failed to bind socket: {e}");
            }
            return ExitCode::FAILURE;
        }
    };

    let layout = env::var("RHISPER_LAYOUT")
        .ok()
        .filter(|l| !l.is_empty())
        .unwrap_or_else(|| "us".to_string());

    // Persist the layout so the orchestrator can detect and restart a stale daemon.
    if let Ok(mut f) = fs::File::create(LAYOUT_FILE) {
        let _ = writeln!(f, "{layout}");
    }

    println!(
        "rhispertoold: listening on {} (layout: {layout})",
        ipc::socket_path().display()
    );

    let mut buf = [0u8; 2];
    loop {
        let n = match socket.recv(&mut buf) {
            Ok(n) => n,
            Err(_) => continue,
        };
        if n == 0 {
            continue;
        }
        match Command::decode(&buf[..n]) {
            Some(Command::Paste) => device.do_paste(),
            Some(Command::Type(c)) => device.type_char(c, &layout),
            Some(Command::Backspace) => device.do_backspace(),
            Some(Command::RightAlt) => device.do_key(uinput::KEY_RIGHTALT),
            Some(Command::LeftAlt) => device.do_key(uinput::KEY_LEFTALT),
            Some(Command::LeftCtrl) => device.do_key(uinput::KEY_LEFTCTRL),
            Some(Command::RightCtrl) => device.do_key(uinput::KEY_RIGHTCTRL),
            Some(Command::LeftShift) => device.do_key(uinput::KEY_LEFTSHIFT),
            Some(Command::RightShift) => device.do_key(uinput::KEY_RIGHTSHIFT),
            Some(Command::Super) => device.do_key(uinput::KEY_LEFTMETA),
            None => {}
        }
    }
}

#[cfg(target_os = "linux")]
fn show_usage() {
    eprintln!(
        "Usage:\n\
         \x20 rhispertool paste            - Paste from clipboard (Ctrl+V)\n\
         \x20 rhispertool type <char>      - Type a single ASCII character\n\
         \x20 rhispertool backspace        - Press backspace\n\
         \n\
         Input switching keys:\n\
         \x20 rhispertool leftalt          - Press left alt\n\
         \x20 rhispertool rightalt         - Press right alt\n\
         \x20 rhispertool leftctrl         - Press left ctrl\n\
         \x20 rhispertool rightctrl        - Press right ctrl\n\
         \x20 rhispertool leftshift        - Press left shift\n\
         \x20 rhispertool rightshift       - Press right shift\n\
         \x20 rhispertool super            - Press super (Windows key)\n\
         \n\
         Daemon:\n\
         \x20 rhispertoold                 - Run daemon (or rhispertool --daemon)"
    );
}

#[cfg(target_os = "linux")]
fn run_client(args: &[String]) -> ExitCode {
    if args.len() < 2 {
        show_usage();
        return ExitCode::FAILURE;
    }

    let command = match args[1].as_str() {
        "paste" => Command::Paste,
        "backspace" => Command::Backspace,
        "rightalt" => Command::RightAlt,
        "leftalt" => Command::LeftAlt,
        "leftctrl" => Command::LeftCtrl,
        "rightctrl" => Command::RightCtrl,
        "leftshift" => Command::LeftShift,
        "rightshift" => Command::RightShift,
        "super" => Command::Super,
        "type" => {
            if args.len() != 3 || args[2].len() != 1 {
                eprintln!("Error: 'type' requires exactly one character argument");
                show_usage();
                return ExitCode::FAILURE;
            }
            Command::Type(args[2].as_bytes()[0])
        }
        other => {
            eprintln!("Error: Unknown command '{other}'");
            show_usage();
            return ExitCode::FAILURE;
        }
    };

    let socket = match ipc::connect_client() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("failed to connect to rhispertoold: {e}");
            match e.raw_os_error() {
                Some(libc::ENOENT) | Some(libc::ECONNREFUSED) => {
                    eprintln!("Please check if rhispertoold is running.");
                    eprintln!("Start it with: rhispertoold &");
                }
                Some(libc::EACCES) | Some(libc::EPERM) => {
                    eprintln!("Permission denied. Check socket permissions.");
                }
                _ => {}
            }
            return ExitCode::from(2);
        }
    };

    let encoded = command.encode();
    let len = command.encoded_len();
    if let Err(e) = socket.send(&encoded[..len]) {
        eprintln!("failed to send command: {e}");
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}
