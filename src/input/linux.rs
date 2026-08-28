// input/linux.rs - Injector adapter over the uinput daemon's socket
// protocol. Pure indirection: every method forwards 1:1 to the existing
// ipc::ToolClient command it already sent before the Injector trait existed.

use crate::ipc::{Command, ToolClient};

use super::{Injector, ModifierKey};

pub struct LinuxInjector(pub ToolClient);

impl Injector for LinuxInjector {
    fn type_ascii_char(&mut self, c: u8) {
        let _ = self.0.type_char(c);
    }

    fn type_text(&mut self, _text: &str) {
        unimplemented!("Linux types char-by-char via type_ascii_char; see paste::chunk_for_typing")
    }

    fn backspace(&mut self) {
        let _ = self.0.backspace();
    }

    fn paste_shortcut(&mut self) {
        let _ = self.0.paste();
    }

    fn press_modifier(&mut self, key: ModifierKey) {
        let command = match key {
            ModifierKey::LeftAlt => Command::LeftAlt,
            ModifierKey::RightAlt => Command::RightAlt,
            ModifierKey::LeftCtrl => Command::LeftCtrl,
            ModifierKey::RightCtrl => Command::RightCtrl,
            ModifierKey::LeftShift => Command::LeftShift,
            ModifierKey::RightShift => Command::RightShift,
            ModifierKey::Super => Command::Super,
        };
        let _ = self.0.press(command);
    }
}
