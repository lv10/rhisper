// clipboard.rs - clipboard read/write for rhisper, using in-process arboard
// instead of shelling out to wl-copy/wl-paste/xclip.
//
// Wayland/X11 clipboard ownership is tied to a live process serving paste
// requests; the shelled-out `wl-copy` this replaces self-forks internally so
// its copy survives after the invoking shell exits. arboard has no such
// auto-daemonizing behavior - SetExtLinux::wait_until() must be used
// explicitly to keep serving requests for a short window from a background
// thread. This is the one area of the rewrite most worth extra manual
// Wayland testing (see the project plan).

use std::thread;
use std::time::{Duration, Instant};

use arboard::{Clipboard, SetExtLinux};

/// How long the background thread keeps serving the clipboard content after
/// a set_text() call, covering the short window between the clipboard write
/// and the target application's Ctrl+V read.
const SERVE_WINDOW: Duration = Duration::from_secs(2);

/// Reads the current clipboard text, or an empty string if the clipboard is
/// empty/unavailable/non-text.
pub fn get_text() -> String {
    Clipboard::new()
        .and_then(|mut c| c.get_text())
        .unwrap_or_default()
}

/// Sets the clipboard to `text` and keeps serving paste requests for it in
/// the background for SERVE_WINDOW, then lets the clipboard fall silent
/// (matching a `wl-copy` process exiting once superseded). Fire-and-forget:
/// callers still need their own short sleep before triggering the paste
/// keystroke - see the non-ascii-*-delay config options.
pub fn set_text(text: String) {
    thread::spawn(move || {
        if let Ok(mut clipboard) = Clipboard::new() {
            let _ = clipboard
                .set()
                .wait_until(Instant::now() + SERVE_WINDOW)
                .text(text);
        }
    });
}

// chunk_for_typing (src/paste.rs) covers the ASCII/Unicode batching logic in
// isolation; clipboard set/get themselves need a real Wayland/X11 session
// and are exercised by manual testing, not CI.
