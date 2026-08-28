// uinput.rs - virtual keyboard device management for rhisper.
// Built on evdev::uinput::VirtualDevice, a safe wrapper around the
// UI_SET_KEYBIT/UI_DEV_SETUP/UI_DEV_CREATE ioctls. Every sleep duration is
// empirically tuned against real compositor race conditions (see the
// Danish AltGr/dead-key handling in type_char()).

use std::io;
use std::thread::sleep;
use std::time::Duration;

use evdev::uinput::VirtualDevice;
use evdev::{AttributeSet, BusType, EventType, InputEvent, InputId, KeyCode};

use super::keymap::{self, FLAG_ALTGR, FLAG_DEADKEY, FLAG_UPPERCASE};

pub const KEY_LEFTCTRL: KeyCode = KeyCode::KEY_LEFTCTRL;
pub const KEY_RIGHTCTRL: KeyCode = KeyCode::KEY_RIGHTCTRL;
pub const KEY_LEFTALT: KeyCode = KeyCode::KEY_LEFTALT;
pub const KEY_RIGHTALT: KeyCode = KeyCode::KEY_RIGHTALT;
pub const KEY_LEFTSHIFT: KeyCode = KeyCode::KEY_LEFTSHIFT;
pub const KEY_RIGHTSHIFT: KeyCode = KeyCode::KEY_RIGHTSHIFT;
pub const KEY_LEFTMETA: KeyCode = KeyCode::KEY_LEFTMETA;
pub const KEY_V: KeyCode = KeyCode::KEY_V;

/// Owns the virtual keyboard device for the lifetime of the daemon.
pub struct RhisperDevice {
    device: VirtualDevice,
}

impl RhisperDevice {
    /// Registers a virtual USB keyboard supporting every key rhisper can
    /// emit.
    pub fn create() -> io::Result<Self> {
        let mut keys = AttributeSet::<KeyCode>::new();

        // Letters (contiguous physical-key ranges in linux/input-event-codes.h)
        for code in KeyCode::KEY_Q.0..=KeyCode::KEY_P.0 {
            keys.insert(KeyCode(code));
        }
        for code in KeyCode::KEY_A.0..=KeyCode::KEY_L.0 {
            keys.insert(KeyCode(code));
        }
        for code in KeyCode::KEY_Z.0..=KeyCode::KEY_M.0 {
            keys.insert(KeyCode(code));
        }

        // Numbers
        for code in KeyCode::KEY_1.0..=KeyCode::KEY_0.0 {
            keys.insert(KeyCode(code));
        }

        // Special keys
        for key in [
            KeyCode::KEY_SPACE,
            KeyCode::KEY_MINUS,
            KeyCode::KEY_EQUAL,
            KeyCode::KEY_LEFTBRACE,
            KeyCode::KEY_RIGHTBRACE,
            KeyCode::KEY_SEMICOLON,
            KeyCode::KEY_APOSTROPHE,
            KeyCode::KEY_GRAVE,
            KeyCode::KEY_BACKSLASH,
            KeyCode::KEY_COMMA,
            KeyCode::KEY_DOT,
            KeyCode::KEY_SLASH,
            KeyCode(keymap::KEY_102ND as u16),
            KeyCode::KEY_TAB,
            KeyCode::KEY_ENTER,
            KeyCode::KEY_BACKSPACE,
        ] {
            keys.insert(key);
        }

        // Modifiers
        for key in [
            KEY_LEFTCTRL,
            KEY_RIGHTCTRL,
            KEY_LEFTALT,
            KEY_RIGHTALT,
            KEY_LEFTSHIFT,
            KEY_RIGHTSHIFT,
            KEY_LEFTMETA,
        ] {
            keys.insert(key);
        }

        let device = VirtualDevice::builder()?
            .name("rhisper")
            .input_id(InputId::new(BusType::BUS_USB, 0x1234, 0x5678, 0))
            .with_keys(&keys)?
            .build()?;

        // Give the kernel/compositor time to notice the new input device
        // before the daemon starts accepting commands.
        sleep(Duration::from_micros(100_000));

        Ok(RhisperDevice { device })
    }

    fn emit(&mut self, code: KeyCode, value: i32) {
        // Best-effort, matching the C implementation's unchecked write().
        let _ = self
            .device
            .emit(&[InputEvent::new(EventType::KEY.0, code.0, value)]);
    }

    pub fn do_paste(&mut self) {
        self.emit(KEY_LEFTCTRL, 1);
        sleep(Duration::from_micros(8000));
        self.emit(KEY_V, 1);
        sleep(Duration::from_micros(8000));
        self.emit(KEY_V, 0);
        sleep(Duration::from_micros(2000));
        self.emit(KEY_LEFTCTRL, 0);
    }

    pub fn type_char(&mut self, c: u8, layout: &str) {
        let kdef = keymap::keymap_lookup(layout, c);
        if kdef == -1 {
            return;
        }

        let keycode = KeyCode((kdef & 0xffff) as u16);
        let shift = kdef & FLAG_UPPERCASE != 0;
        let altgr = kdef & FLAG_ALTGR != 0;
        let dead = kdef & FLAG_DEADKEY != 0;

        if shift {
            self.emit(KEY_LEFTSHIFT, 1);
            sleep(Duration::from_micros(2000));
        }
        if altgr {
            self.emit(KEY_RIGHTALT, 1);
            sleep(Duration::from_micros(2000));
        }

        self.emit(keycode, 1);
        sleep(Duration::from_micros(8000));
        self.emit(keycode, 0);
        sleep(Duration::from_micros(2000));

        if altgr {
            self.emit(KEY_RIGHTALT, 0);
        }
        if shift {
            self.emit(KEY_LEFTSHIFT, 0);
        }

        // Dead keys only produce a character when followed by a key;
        // Space resolves them to the standalone character.
        if dead {
            self.emit(KeyCode::KEY_SPACE, 1);
            sleep(Duration::from_micros(2000));
            self.emit(KeyCode::KEY_SPACE, 0);
        }
    }

    pub fn do_backspace(&mut self) {
        self.emit(KeyCode::KEY_BACKSPACE, 1);
        sleep(Duration::from_micros(8000));
        self.emit(KeyCode::KEY_BACKSPACE, 0);
    }

    pub fn do_key(&mut self, keycode: KeyCode) {
        self.emit(keycode, 1);
        sleep(Duration::from_micros(8000));
        self.emit(keycode, 0);
    }
}
