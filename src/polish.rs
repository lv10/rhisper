// polish.rs - light cleanup of dictated text, shaped by where it is typed.
//
// With polishing on, the transcription request asks the model to drop slips
// (fillers, false starts, self-corrections) and group ideas into paragraphs.
// What is acceptable to type depends on the destination: a line break is an
// Enter keypress, which would run a command in a terminal or send a message
// in a chat box, and lists only make sense in apps built for prose. The
// destination is therefore reduced to a coarse kind that gates both, and
// `finalize` enforces it on the model's output rather than trusting the
// prompt alone.

use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use crate::config::{PolishSetting, Provider};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DestinationKind {
    Terminal,
    Browser,
    Email,
    Other,
    Unknown,
}

impl DestinationKind {
    pub fn allows_line_breaks(self) -> bool {
        matches!(self, Self::Browser | Self::Email | Self::Other)
    }

    pub fn allows_lists(self) -> bool {
        matches!(self, Self::Browser | Self::Email)
    }
}

#[derive(Debug, Clone)]
pub struct Destination {
    /// Application name or window class as reported; empty when unknown.
    /// Never a window title.
    pub app: String,
    pub kind: DestinationKind,
}

#[derive(Debug, Clone)]
pub struct PolishContext {
    pub destination: Destination,
}

/// `On` and `Auto` both mean "polish if the provider can"; only Gemini can
/// (it transcribes and polishes in one request), `Off` always wins.
pub fn polish_active(setting: PolishSetting, provider: Provider) -> bool {
    setting != PolishSetting::Off && provider == Provider::Gemini
}

/// Builds the context for one dictation, or None when polishing is off for
/// this config. Detection runs here, just before the request, so the
/// destination at insertion time is the one used.
pub fn build_polish_context(setting: PolishSetting, provider: Provider) -> Option<PolishContext> {
    polish_active(setting, provider).then(|| PolishContext {
        destination: detect(),
    })
}

/// The per-destination formatting guidance sent beside the audio.
pub fn formatting_line(kind: DestinationKind) -> &'static str {
    match kind {
        DestinationKind::Terminal | DestinationKind::Unknown => {
            "Output a single line of plain text: no line breaks, no lists."
        }
        DestinationKind::Other => "Plain paragraphs are fine; do not use lists.",
        DestinationKind::Browser | DestinationKind::Email => {
            "Paragraphs are fine. If the speaker lists three or more items or steps, \
format them as a bullet list, or a numbered list if the order matters; otherwise \
do not use a list."
        }
    }
}

/// Enforces the destination's rules on the model's output: the prompt asks
/// nicely, this guarantees it. A line break typed into a terminal presses
/// Enter, so none may survive there.
pub fn finalize(kind: DestinationKind, text: &str) -> String {
    let lines: Vec<&str> = text
        .lines()
        .map(|l| {
            if kind.allows_lists() {
                l.trim_end()
            } else {
                strip_list_marker(l.trim_end())
            }
        })
        .collect();

    if !kind.allows_line_breaks() {
        return lines
            .iter()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
    }

    // At most one blank line between paragraphs.
    let mut out: Vec<&str> = Vec::new();
    for line in lines {
        if line.trim().is_empty() && out.last().is_none_or(|l| l.is_empty()) {
            continue;
        }
        out.push(if line.trim().is_empty() { "" } else { line });
    }
    out.join("\n").trim().to_string()
}

/// Drops a leading `- `, `* `, `• `, `1. ` or `1) ` marker from a line.
fn strip_list_marker(line: &str) -> &str {
    let t = line.trim_start();
    for bullet in ["- ", "* ", "• "] {
        if let Some(rest) = t.strip_prefix(bullet) {
            return rest;
        }
    }
    let digits = t.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits > 0 {
        if let Some(rest) = t[digits..]
            .strip_prefix(". ")
            .or_else(|| t[digits..].strip_prefix(") "))
        {
            return rest;
        }
    }
    line
}

const TERMINALS: &[&str] = &[
    "terminal",
    "kitty",
    "alacritty",
    "wezterm",
    "foot",
    "konsole",
    "xterm",
    "terminator",
    "tilix",
    "ghostty",
    "iterm",
    "warp",
];
const EMAIL_CLIENTS: &[&str] = &[
    "thunderbird",
    "evolution",
    "geary",
    "kmail",
    "outlook",
    "mail",
];
const BROWSERS: &[&str] = &[
    "firefox",
    "chrome",
    "chromium",
    "brave",
    "vivaldi",
    "zen",
    "librewolf",
    "safari",
    "edge",
    "arc",
];

/// Maps an application name or window class to a coarse kind. Terminals are
/// checked first because they are the destinations where a wrong guess does
/// damage (an Enter keypress runs a command).
pub fn classify(app: &str) -> DestinationKind {
    let name = app.trim().to_ascii_lowercase();
    if name.is_empty() {
        return DestinationKind::Unknown;
    }
    let matches = |list: &[&str]| list.iter().any(|k| name.contains(k));
    if matches(TERMINALS) {
        DestinationKind::Terminal
    } else if matches(EMAIL_CLIENTS) {
        DestinationKind::Email
    } else if matches(BROWSERS) {
        DestinationKind::Browser
    } else {
        DestinationKind::Other
    }
}

/// Finds the focused application and classifies it. Best effort: any
/// failure yields an unknown destination, which is formatted as plain text.
pub fn detect() -> Destination {
    let app = detect_app();
    Destination {
        kind: classify(&app),
        app,
    }
}

/// How long any single detector may run; dictation must not stall on a
/// missing or slow desktop tool.
const DETECT_TIMEOUT: Duration = Duration::from_millis(300);

/// Runs `program`, returning trimmed stdout only if it exits 0 with output
/// inside the time limit. stdout is drained on a thread so a large reply
/// (a Sway tree) cannot fill the pipe and stall the child.
fn run_with_timeout(program: &str, args: &[&str], limit: Duration) -> Option<String> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut stdout = child.stdout.take()?;
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = String::new();
        let _ = stdout.read_to_string(&mut buf);
        let _ = tx.send(buf);
    });

    match rx.recv_timeout(limit) {
        Ok(out) => {
            let ok = child.wait().is_ok_and(|s| s.success());
            let out = out.trim().to_string();
            (ok && !out.is_empty()).then_some(out)
        }
        Err(_) => {
            let _ = child.kill();
            let _ = child.wait();
            None
        }
    }
}

fn parse_hyprland(json: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let class = v.get("class")?.as_str()?.trim();
    (!class.is_empty()).then(|| class.to_string())
}

fn parse_sway(json: &str) -> Option<String> {
    fn focused(node: &serde_json::Value) -> Option<String> {
        if node.get("focused").and_then(|f| f.as_bool()) == Some(true) {
            let app_id = node.get("app_id").and_then(|a| a.as_str()).unwrap_or("");
            if !app_id.is_empty() {
                return Some(app_id.to_string());
            }
            // XWayland windows have no app_id.
            let class = node
                .pointer("/window_properties/class")
                .and_then(|c| c.as_str())
                .unwrap_or("");
            return (!class.is_empty()).then(|| class.to_string());
        }
        ["nodes", "floating_nodes"]
            .iter()
            .filter_map(|k| node.get(*k)?.as_array())
            .flatten()
            .find_map(focused)
    }
    focused(&serde_json::from_str(json).ok()?)
}

#[cfg(target_os = "linux")]
fn detect_app() -> String {
    let has = |var: &str| std::env::var_os(var).is_some_and(|v| !v.is_empty());
    let run = |program: &str, args: &[&str]| run_with_timeout(program, args, DETECT_TIMEOUT);

    if has("HYPRLAND_INSTANCE_SIGNATURE") {
        if let Some(app) = run("hyprctl", &["activewindow", "-j"]).and_then(|j| parse_hyprland(&j))
        {
            return app;
        }
    }
    if has("SWAYSOCK") {
        if let Some(app) = run("swaymsg", &["-t", "get_tree"]).and_then(|j| parse_sway(&j)) {
            return app;
        }
    }
    if let Some(app) = run("kdotool", &["getactivewindow", "getwindowclassname"]) {
        return app;
    }
    if has("DISPLAY") {
        if let Some(app) = run("xdotool", &["getactivewindow", "getwindowclassname"]) {
            return app;
        }
    }
    String::new()
}

#[cfg(target_os = "macos")]
fn detect_app() -> String {
    run_with_timeout(
        "osascript",
        &[
            "-e",
            "tell application \"System Events\" to get name of first process whose frontmost is true",
        ],
        DETECT_TIMEOUT,
    )
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polish_is_gemini_only_and_off_always_wins() {
        assert!(polish_active(PolishSetting::Auto, Provider::Gemini));
        assert!(polish_active(PolishSetting::On, Provider::Gemini));
        assert!(!polish_active(PolishSetting::Off, Provider::Gemini));
        assert!(!polish_active(PolishSetting::Auto, Provider::Groq));
        assert!(!polish_active(PolishSetting::On, Provider::Groq));
        assert!(!polish_active(PolishSetting::Off, Provider::Groq));
    }

    #[test]
    fn only_prose_destinations_allow_line_breaks_and_lists() {
        use DestinationKind::*;
        for (kind, breaks, lists) in [
            (Terminal, false, false),
            (Unknown, false, false),
            (Other, true, false),
            (Browser, true, true),
            (Email, true, true),
        ] {
            assert_eq!(kind.allows_line_breaks(), breaks, "{kind:?}");
            assert_eq!(kind.allows_lists(), lists, "{kind:?}");
        }
    }
}

#[test]
fn finalize_collapses_line_breaks_for_terminal_and_unknown() {
    for kind in [DestinationKind::Terminal, DestinationKind::Unknown] {
        assert_eq!(finalize(kind, "  one\n\ntwo \n"), "one two");
    }
}

#[test]
fn finalize_strips_list_markers_when_lists_are_not_allowed() {
    let text = "- milk\n* eggs\n• bread\n1. a\n2) b";
    assert_eq!(
        finalize(DestinationKind::Terminal, text),
        "milk eggs bread a b"
    );
    assert_eq!(
        finalize(DestinationKind::Other, text),
        "milk\neggs\nbread\na\nb"
    );
}

#[test]
fn finalize_leaves_plain_single_line_text_alone() {
    assert_eq!(
        finalize(DestinationKind::Terminal, "Hello, my name is Dan Peter."),
        "Hello, my name is Dan Peter."
    );
    assert_eq!(
        finalize(DestinationKind::Unknown, "2024 was good"),
        "2024 was good"
    );
}

#[test]
fn finalize_keeps_paragraphs_where_line_breaks_are_allowed() {
    for kind in [
        DestinationKind::Other,
        DestinationKind::Browser,
        DestinationKind::Email,
    ] {
        assert_eq!(finalize(kind, "one\n\ntwo"), "one\n\ntwo");
    }
    assert_eq!(
        finalize(DestinationKind::Email, "one\n\n\n\ntwo\n"),
        "one\n\ntwo"
    );
}

#[test]
fn finalize_keeps_lists_for_browser_and_email() {
    let text = "Shopping:\n- milk\n- eggs\n1. first";
    assert_eq!(finalize(DestinationKind::Browser, text), text);
    assert_eq!(finalize(DestinationKind::Email, text), text);
}

#[test]
fn formatting_lines_match_the_contract() {
    assert!(formatting_line(DestinationKind::Terminal).contains("single line"));
    assert!(formatting_line(DestinationKind::Unknown).contains("no lists"));
    assert_eq!(
        formatting_line(DestinationKind::Other),
        "Plain paragraphs are fine; do not use lists."
    );
    for kind in [DestinationKind::Browser, DestinationKind::Email] {
        let line = formatting_line(kind);
        assert!(line.contains("bullet list") && line.contains("numbered list"));
    }
}

#[test]
fn classify_known_applications() {
    use DestinationKind::*;
    for (app, kind) in [
        ("kitty", Terminal),
        ("Alacritty", Terminal),
        ("gnome-terminal-server", Terminal),
        ("iTerm2", Terminal),
        ("Terminal", Terminal),
        ("thunderbird", Email),
        ("Mail", Email),
        ("org.gnome.Evolution", Email),
        ("firefox", Browser),
        ("Google-chrome", Browser),
        ("Safari", Browser),
        ("zen", Browser),
        ("code", Other),
        ("Slack", Other),
        ("", Unknown),
        ("   ", Unknown),
    ] {
        assert_eq!(classify(app), kind, "{app}");
    }
}

#[test]
fn terminal_wins_when_a_name_matches_several_lists() {
    assert_eq!(classify("firefox-terminal"), DestinationKind::Terminal);
}

#[test]
fn parses_hyprland_activewindow() {
    assert_eq!(
        parse_hyprland(r#"{"address":"0x1","class":"firefox","title":"secret"}"#),
        Some("firefox".to_string())
    );
    assert_eq!(parse_hyprland(r#"{"class":""}"#), None);
    assert_eq!(parse_hyprland("not json"), None);
}

#[test]
fn parses_the_focused_node_of_a_sway_tree() {
    let tree = r#"{"focused":false,"nodes":[
            {"focused":false,"app_id":"kitty","nodes":[]},
            {"focused":false,"nodes":[
                {"focused":true,"app_id":"thunderbird","name":"secret","nodes":[]}]}],
            "floating_nodes":[]}"#;
    assert_eq!(parse_sway(tree), Some("thunderbird".to_string()));

    let xwayland = r#"{"focused":false,"nodes":[{"focused":true,"app_id":null,
            "window_properties":{"class":"Firefox"},"nodes":[]}]}"#;
    assert_eq!(parse_sway(xwayland), Some("Firefox".to_string()));

    assert_eq!(parse_sway(r#"{"focused":false,"nodes":[]}"#), None);
}

#[test]
fn run_with_timeout_returns_output_and_enforces_the_limit() {
    let limit = Duration::from_secs(2);
    assert_eq!(
        run_with_timeout("echo", &["firefox"], limit),
        Some("firefox".to_string())
    );
    assert_eq!(run_with_timeout("false", &[], limit), None);
    assert_eq!(run_with_timeout("rhisper-no-such-tool", &[], limit), None);
    let started = std::time::Instant::now();
    assert_eq!(
        run_with_timeout("sleep", &["5"], Duration::from_millis(100)),
        None
    );
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[test]
fn no_context_is_built_when_polish_is_inactive() {
    assert!(build_polish_context(PolishSetting::Off, Provider::Gemini).is_none());
    assert!(build_polish_context(PolishSetting::Auto, Provider::Groq).is_none());
}
