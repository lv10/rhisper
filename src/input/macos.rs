// input/macos.rs - Injector implementation using CoreGraphics event posting.
//
// CGEventPost is a stateless one-shot call - no persistent virtual device to
// register, unlike Linux's uinput - so there's no daemon here, just direct
// in-process event posting. Typing uses CGEventKeyboardSetUnicodeString
// (CGEvent::set_string) rather than simulating physical keypresses, which
// works correctly regardless of the active keyboard layout and makes the
// per-layout keymap tables Linux needs unnecessary here.

use core_graphics::event::{
    CGEvent, CGEventFlags, CGEventTapLocation, CGKeyCode, KeyCode as CgKey,
};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

use super::{Injector, ModifierKey};

pub struct CgInjector {
    source: CGEventSource,
}

impl CgInjector {
    pub fn new() -> Result<Self, String> {
        CGEventSource::new(CGEventSourceStateID::HIDSystemState)
            .map(|source| CgInjector { source })
            .map_err(|_| "failed to create CGEventSource".to_string())
    }

    fn post_key(&self, keycode: CGKeyCode, down: bool, flags: CGEventFlags) {
        if let Ok(event) = CGEvent::new_keyboard_event(self.source.clone(), keycode, down) {
            event.set_flags(flags);
            event.post(CGEventTapLocation::HID);
        }
    }
}

impl Injector for CgInjector {
    fn type_ascii_char(&mut self, c: u8) {
        self.type_text(&(c as char).to_string());
    }

    fn type_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        // A keycode-0 keyboard event carrying the whole string types
        // correctly regardless of the active input source - no per-layout
        // table needed, unlike Linux's physical-scancode simulation.
        if let Ok(event) = CGEvent::new_keyboard_event(self.source.clone(), 0, true) {
            event.set_string(text);
            event.post(CGEventTapLocation::HID);
        }
        if let Ok(event) = CGEvent::new_keyboard_event(self.source.clone(), 0, false) {
            event.post(CGEventTapLocation::HID);
        }
    }

    fn backspace(&mut self) {
        self.post_key(CgKey::DELETE, true, CGEventFlags::empty());
        self.post_key(CgKey::DELETE, false, CGEventFlags::empty());
    }

    fn paste_shortcut(&mut self) {
        self.post_key(CgKey::ANSI_V, true, CGEventFlags::CGEventFlagCommand);
        self.post_key(CgKey::ANSI_V, false, CGEventFlags::CGEventFlagCommand);
    }

    fn press_modifier(&mut self, key: ModifierKey) {
        let code = match key {
            ModifierKey::LeftAlt => CgKey::OPTION,
            ModifierKey::RightAlt => CgKey::RIGHT_OPTION,
            ModifierKey::LeftCtrl => CgKey::CONTROL,
            ModifierKey::RightCtrl => CgKey::RIGHT_CONTROL,
            ModifierKey::LeftShift => CgKey::SHIFT,
            ModifierKey::RightShift => CgKey::RIGHT_SHIFT,
            // macOS has no physical Super/Windows key; Command is the
            // closest analog. The original Linux use case (wrapping typed
            // text around an input-source-switch key) doesn't really apply
            // here since type_text() bypasses layout switching entirely,
            // but this keeps the flag functionally alive rather than a
            // silent no-op.
            ModifierKey::Super => CgKey::COMMAND,
        };
        self.post_key(code, true, CGEventFlags::empty());
        self.post_key(code, false, CGEventFlags::empty());
    }
}
