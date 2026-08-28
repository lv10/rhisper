// input/mod.rs - the keystroke-injection abstraction shared by both
// platforms.
//
// Linux injects keystrokes by talking to a persistent uinput daemon over a
// socket (see `linux`), because creating a virtual /dev/uinput device has a
// real registration cost worth amortizing across invocations. macOS's
// CGEventPost is a stateless one-shot call with no such cost, so it injects
// directly in-process (see `macos`) with no daemon at all. `Injector` is the
// one shared surface the orchestration logic in `bin/rhisper.rs` depends on,
// so that split is isolated here rather than scattered through the caller.

#[cfg(target_os = "linux")]
pub mod keymap;
#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "linux")]
pub mod uinput;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModifierKey {
    LeftAlt,
    RightAlt,
    LeftCtrl,
    RightCtrl,
    LeftShift,
    RightShift,
    Super,
}

/// Everything the client binary needs to inject keystrokes at the cursor,
/// abstracted over "talk to the uinput daemon over a socket" (Linux) vs
/// "post CGEvents in-process" (macOS).
pub trait Injector {
    /// Types a single printable-ASCII character, layout-sensitive on Linux.
    fn type_ascii_char(&mut self, c: u8);
    /// Types an entire string in one shot. macOS-only fast path: CoreGraphics
    /// can set a Unicode string directly on a synthetic key event regardless
    /// of the active keyboard layout, so this bypasses per-char/layout logic
    /// entirely. Linux has no equivalent and never calls this.
    fn type_text(&mut self, text: &str);
    fn backspace(&mut self);
    /// The platform paste shortcut (Ctrl+V on Linux, Cmd+V on macOS).
    fn paste_shortcut(&mut self);
    fn press_modifier(&mut self, key: ModifierKey);
}
