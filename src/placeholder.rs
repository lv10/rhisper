// placeholder.rs - progress feedback while recording and transcribing.
//
// The status placeholders ("recording", "transcribing", "no sound") can
// either be typed into the target field and deleted again, or shown as a
// desktop notification. Inline typing is what rhisper has always done, but
// it briefly writes into whatever the user is editing and relies on sending
// exactly as many backspaces as characters typed - which goes wrong the
// moment the target field autocompletes, reformats, or swallows a keystroke.
// Notifications sidestep that entirely at the cost of a `notify-send`
// (libnotify) and `gdbus` (glib) dependency, so they are opt-out.

use std::fs;
use std::process::{Command, Stdio};

use crate::config::PasteMode;

const NOTIFY_ID_FILE: &str = "/tmp/rhisper-notify-id";

/// What the user asked for in rhisperrc.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Setting {
    Auto,
    Inline,
    Notify,
    Off,
}

/// What that resolves to for this run - `Auto` is gone by then.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Inline,
    Notify,
    Off,
}

/// Resolves `auto` to notifications whenever the paste mode is one of the
/// clipboard ones and notifications are actually available.
///
/// The paste mode matters because it decides how the placeholder would be
/// removed again: in `type` mode the text is typed key by key and the
/// backspaces that delete it are just as reliable as the typing was, while
/// the clipboard modes paste in one go into a field that may react to it,
/// leaving the backspace count a guess.
pub fn resolve(setting: Setting, paste_mode: PasteMode, notifications_available: bool) -> Mode {
    match setting {
        Setting::Inline => Mode::Inline,
        Setting::Notify => Mode::Notify,
        Setting::Off => Mode::Off,
        Setting::Auto => {
            if paste_mode != PasteMode::Type && notifications_available {
                Mode::Notify
            } else {
                Mode::Inline
            }
        }
    }
}

/// Whether desktop notifications can be shown at all. macOS has no
/// `notify-send`, so `auto` resolves to inline there.
pub fn notifications_available() -> bool {
    which("notify-send")
}

fn which(binary: &str) -> bool {
    Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {binary}"))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Posts a notification, replacing the one from an earlier step of the same
/// dictation so the desktop shows one updating bubble rather than a stack.
///
/// `persistent` keeps the bubble up until [`dismiss`] closes it (progress
/// steps); otherwise it expires on its own, which is what the closing
/// messages want - nobody is left to dismiss them.
pub fn notify(message: &str, persistent: bool) {
    let mut cmd = Command::new("notify-send");
    cmd.arg("--print-id")
        .args(["-i", "audio-input-microphone"])
        .args(["-t", if persistent { "0" } else { "3000" }]);

    if let Some(id) = previous_id() {
        cmd.args(["-r", &id]);
    }

    let output = cmd
        .arg("rhisper")
        .arg(message)
        .stderr(Stdio::null())
        .output();

    match output {
        Ok(o) if o.status.success() => {
            let id = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if persistent && !id.is_empty() {
                let _ = fs::write(NOTIFY_ID_FILE, id);
            } else {
                // A self-expiring bubble must not be closed later by id: the
                // id would by then likely belong to someone else's notification.
                let _ = fs::remove_file(NOTIFY_ID_FILE);
            }
        }
        _ => {
            let _ = fs::remove_file(NOTIFY_ID_FILE);
        }
    }
}

/// Closes the notification left standing by [`notify`], if any.
pub fn dismiss() {
    let Some(id) = previous_id() else {
        return;
    };
    let _ = Command::new("gdbus")
        .args([
            "call",
            "--session",
            "--dest",
            "org.freedesktop.Notifications",
            "--object-path",
            "/org/freedesktop/Notifications",
            "--method",
            "org.freedesktop.Notifications.CloseNotification",
            &id,
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let _ = fs::remove_file(NOTIFY_ID_FILE);
}

fn previous_id() -> Option<String> {
    let id = fs::read_to_string(NOTIFY_ID_FILE).ok()?;
    let id = id.trim();
    (!id.is_empty() && id.chars().all(|c| c.is_ascii_digit())).then(|| id.to_string())
}

/// Substitutes `${...}` variables in a placeholder string.
///
/// Only a fixed set of names is recognised; anything else is left standing
/// verbatim, so a stray `${` in a status text cannot swallow the rest of the
/// line or turn into an empty string the user cannot explain.
pub fn expand(template: &str, device: &str) -> String {
    if !template.contains("${") {
        return template.to_string();
    }

    let mut out = String::with_capacity(template.len());
    let mut rest = template;

    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];

        let Some(end) = after.find('}') else {
            // Unterminated - the rest is literal text.
            out.push_str(&rest[start..]);
            return out;
        };

        match &after[..end] {
            "device" => out.push_str(device),
            _ => out.push_str(&rest[start..start + 2 + end + 1]),
        }
        rest = &after[end + 1..];
    }

    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_prefers_notifications_for_clipboard_pastes() {
        assert_eq!(
            resolve(Setting::Auto, PasteMode::ClipboardRestore, true),
            Mode::Notify
        );
        assert_eq!(
            resolve(Setting::Auto, PasteMode::Clipboard, true),
            Mode::Notify
        );
    }

    #[test]
    fn auto_stays_inline_when_typing_or_without_notify_send() {
        assert_eq!(resolve(Setting::Auto, PasteMode::Type, true), Mode::Inline);
        assert_eq!(
            resolve(Setting::Auto, PasteMode::ClipboardRestore, false),
            Mode::Inline
        );
    }

    #[test]
    fn expands_the_device_variable() {
        assert_eq!(
            expand("🎤 ${device}", "Anua Mic CM 900"),
            "🎤 Anua Mic CM 900"
        );
        assert_eq!(expand("${device} hört zu", "Webcam"), "Webcam hört zu");
    }

    #[test]
    fn leaves_plain_strings_and_unknown_names_alone() {
        assert_eq!(expand("(recording...)", "Mic"), "(recording...)");
        assert_eq!(expand("${nope}", "Mic"), "${nope}");
        assert_eq!(expand("cost: ${5}", "Mic"), "cost: ${5}");
    }

    #[test]
    fn an_unterminated_variable_stays_literal() {
        // Better a visibly odd status than a silently truncated one.
        assert_eq!(expand("🎤 ${device", "Mic"), "🎤 ${device");
        assert_eq!(expand("${", "Mic"), "${");
    }

    #[test]
    fn expands_every_occurrence_and_survives_an_unknown_device() {
        assert_eq!(expand("${device}/${device}", "A"), "A/A");
        assert_eq!(expand("🎤 ${device}", ""), "🎤 ");
    }

    #[test]
    fn explicit_settings_ignore_paste_mode_and_availability() {
        assert_eq!(
            resolve(Setting::Notify, PasteMode::Type, false),
            Mode::Notify
        );
        assert_eq!(
            resolve(Setting::Inline, PasteMode::ClipboardRestore, true),
            Mode::Inline
        );
        assert_eq!(resolve(Setting::Off, PasteMode::Type, true), Mode::Off);
    }
}
