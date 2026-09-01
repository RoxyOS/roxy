#![no_std]

use pc_keyboard::{EventDecoder as PcDecoder, HandleControl, layouts::Us104Key};
use roxy_input::{KeyCode, KeyEvent, KeyState};

/// A decoded key from the US 104-key layout engine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodedKey {
    /// A printable character, control character, or whitespace.
    Character(char),
    /// A physical key that has no Unicode representation (navigation, function, etc.).
    Key(KeyCode),
}

/// A stateful US 104-key layout decoder that turns raw key events into characters or
/// special-key identifiers.
///
/// The decoder maintains modifier state internally (Shift, Ctrl, `CapsLock`, etc.), so every
/// key press and release **must** be fed through `decode` in order. Modifier-key releases
/// update state only and return `None`; ordinary-key releases also return `None` because the
/// pc-keyboard engine discards them.
pub struct KeyDecoder {
    inner: PcDecoder<Us104Key>,
}

impl KeyDecoder {
    /// Creates a new decoder with the US 104-key layout and `MapLettersToUnicode` Ctrl
    /// handling (Ctrl+C → `\x03`, like the original ps2 driver).
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: PcDecoder::new(Us104Key, HandleControl::MapLettersToUnicode),
        }
    }

    /// Feeds one raw key event through the layout engine.
    ///
    /// Returns `None` for key releases (which only update modifier state) and for keys that
    /// have no visible output (e.g. modifier presses).
    pub fn decode(&mut self, event: KeyEvent) -> Option<DecodedKey> {
        let pc_event = pc_keyboard::KeyEvent::new(
            to_pc_code(event.code),
            match event.state {
                KeyState::Pressed => pc_keyboard::KeyState::Down,
                KeyState::Released => pc_keyboard::KeyState::Up,
            },
        );
        self.inner
            .process_keyevent(pc_event)
            .and_then(|decoded| match decoded {
                pc_keyboard::DecodedKey::Unicode(c) => Some(DecodedKey::Character(c)),
                pc_keyboard::DecodedKey::RawKey(code) => from_pc_code(code).map(DecodedKey::Key),
            })
    }
}

impl Default for KeyDecoder {
    fn default() -> Self {
        Self::new()
    }
}

// --------------------------------------------------------------------------
// roxy KeyCode ↔ pc_keyboard::KeyCode mapping
// --------------------------------------------------------------------------

fn to_pc_code(code: KeyCode) -> pc_keyboard::KeyCode {
    to_pc_function_row(code)
        .or_else(|| to_pc_number_row(code))
        .or_else(|| to_pc_letter_rows(code))
        .or_else(|| to_pc_modifier_row(code))
        .or_else(|| to_pc_editing_and_numpad(code))
        .expect("every Roxy key code maps to a pc-keyboard key code")
}

fn to_pc_function_row(code: KeyCode) -> Option<pc_keyboard::KeyCode> {
    use pc_keyboard::KeyCode as Pc;
    Some(match code {
        KeyCode::Escape => Pc::Escape,
        KeyCode::F1 => Pc::F1,
        KeyCode::F2 => Pc::F2,
        KeyCode::F3 => Pc::F3,
        KeyCode::F4 => Pc::F4,
        KeyCode::F5 => Pc::F5,
        KeyCode::F6 => Pc::F6,
        KeyCode::F7 => Pc::F7,
        KeyCode::F8 => Pc::F8,
        KeyCode::F9 => Pc::F9,
        KeyCode::F10 => Pc::F10,
        KeyCode::F11 => Pc::F11,
        KeyCode::F12 => Pc::F12,
        KeyCode::PrintScreen => Pc::PrintScreen,
        KeyCode::ScrollLock => Pc::ScrollLock,
        KeyCode::PauseBreak => Pc::PauseBreak,
        _ => return None,
    })
}

fn to_pc_number_row(code: KeyCode) -> Option<pc_keyboard::KeyCode> {
    use pc_keyboard::KeyCode as Pc;
    Some(match code {
        KeyCode::Backquote => Pc::Oem8,
        KeyCode::Digit1 => Pc::Key1,
        KeyCode::Digit2 => Pc::Key2,
        KeyCode::Digit3 => Pc::Key3,
        KeyCode::Digit4 => Pc::Key4,
        KeyCode::Digit5 => Pc::Key5,
        KeyCode::Digit6 => Pc::Key6,
        KeyCode::Digit7 => Pc::Key7,
        KeyCode::Digit8 => Pc::Key8,
        KeyCode::Digit9 => Pc::Key9,
        KeyCode::Digit0 => Pc::Key0,
        KeyCode::Minus => Pc::OemMinus,
        KeyCode::Equals => Pc::OemPlus,
        KeyCode::Backspace => Pc::Backspace,
        _ => return None,
    })
}

fn to_pc_letter_rows(code: KeyCode) -> Option<pc_keyboard::KeyCode> {
    use pc_keyboard::KeyCode as Pc;
    Some(match code {
        KeyCode::Tab => Pc::Tab,
        KeyCode::Q => Pc::Q,
        KeyCode::W => Pc::W,
        KeyCode::E => Pc::E,
        KeyCode::R => Pc::R,
        KeyCode::T => Pc::T,
        KeyCode::Y => Pc::Y,
        KeyCode::U => Pc::U,
        KeyCode::I => Pc::I,
        KeyCode::O => Pc::O,
        KeyCode::P => Pc::P,
        KeyCode::BracketLeft => Pc::Oem4,
        KeyCode::BracketRight => Pc::Oem6,
        KeyCode::Backslash => Pc::Oem5,
        KeyCode::CapsLock => Pc::CapsLock,
        KeyCode::A => Pc::A,
        KeyCode::S => Pc::S,
        KeyCode::D => Pc::D,
        KeyCode::F => Pc::F,
        KeyCode::G => Pc::G,
        KeyCode::H => Pc::H,
        KeyCode::J => Pc::J,
        KeyCode::K => Pc::K,
        KeyCode::L => Pc::L,
        KeyCode::Semicolon => Pc::Oem1,
        KeyCode::Apostrophe => Pc::Oem3,
        KeyCode::Return => Pc::Return,
        KeyCode::LeftShift => Pc::LShift,
        KeyCode::Z => Pc::Z,
        KeyCode::X => Pc::X,
        KeyCode::C => Pc::C,
        KeyCode::V => Pc::V,
        KeyCode::B => Pc::B,
        KeyCode::N => Pc::N,
        KeyCode::M => Pc::M,
        KeyCode::Comma => Pc::OemComma,
        KeyCode::Period => Pc::OemPeriod,
        KeyCode::Slash => Pc::Oem2,
        KeyCode::RightShift => Pc::RShift,
        _ => return None,
    })
}

fn to_pc_modifier_row(code: KeyCode) -> Option<pc_keyboard::KeyCode> {
    use pc_keyboard::KeyCode as Pc;
    Some(match code {
        KeyCode::LeftCtrl => Pc::LControl,
        KeyCode::LeftSuper => Pc::LWin,
        KeyCode::LeftAlt => Pc::LAlt,
        KeyCode::Space => Pc::Spacebar,
        KeyCode::RightAlt => Pc::RAltGr,
        KeyCode::RightSuper => Pc::RWin,
        KeyCode::Menu => Pc::Apps,
        KeyCode::RightCtrl => Pc::RControl,
        _ => return None,
    })
}

fn to_pc_editing_and_numpad(code: KeyCode) -> Option<pc_keyboard::KeyCode> {
    use pc_keyboard::KeyCode as Pc;
    Some(match code {
        KeyCode::Insert => Pc::Insert,
        KeyCode::Home => Pc::Home,
        KeyCode::PageUp => Pc::PageUp,
        KeyCode::Delete => Pc::Delete,
        KeyCode::End => Pc::End,
        KeyCode::PageDown => Pc::PageDown,
        KeyCode::ArrowUp => Pc::ArrowUp,
        KeyCode::ArrowDown => Pc::ArrowDown,
        KeyCode::ArrowLeft => Pc::ArrowLeft,
        KeyCode::ArrowRight => Pc::ArrowRight,
        KeyCode::NumpadLock => Pc::NumpadLock,
        KeyCode::NumpadDivide => Pc::NumpadDivide,
        KeyCode::NumpadMultiply => Pc::NumpadMultiply,
        KeyCode::NumpadSubtract => Pc::NumpadSubtract,
        KeyCode::NumpadAdd => Pc::NumpadAdd,
        KeyCode::NumpadEnter => Pc::NumpadEnter,
        KeyCode::NumpadDecimal => Pc::NumpadPeriod,
        KeyCode::Numpad0 => Pc::Numpad0,
        KeyCode::Numpad1 => Pc::Numpad1,
        KeyCode::Numpad2 => Pc::Numpad2,
        KeyCode::Numpad3 => Pc::Numpad3,
        KeyCode::Numpad4 => Pc::Numpad4,
        KeyCode::Numpad5 => Pc::Numpad5,
        KeyCode::Numpad6 => Pc::Numpad6,
        KeyCode::Numpad7 => Pc::Numpad7,
        KeyCode::Numpad8 => Pc::Numpad8,
        KeyCode::Numpad9 => Pc::Numpad9,
        _ => return None,
    })
}

/// Maps a pc-keyboard key code to a Roxy key code.
///
/// Keys that are not present on a US 104-key keyboard (ISO-only, media, protocol states)
/// map to `None`.
fn from_pc_code(code: pc_keyboard::KeyCode) -> Option<KeyCode> {
    from_pc_function_row(code)
        .or_else(|| from_pc_number_row(code))
        .or_else(|| from_pc_letter_rows(code))
        .or_else(|| from_pc_modifier_row(code))
        .or_else(|| from_pc_editing_and_numpad(code))
}

fn from_pc_function_row(code: pc_keyboard::KeyCode) -> Option<KeyCode> {
    use pc_keyboard::KeyCode as Pc;
    Some(match code {
        Pc::Escape => KeyCode::Escape,
        Pc::F1 => KeyCode::F1,
        Pc::F2 => KeyCode::F2,
        Pc::F3 => KeyCode::F3,
        Pc::F4 => KeyCode::F4,
        Pc::F5 => KeyCode::F5,
        Pc::F6 => KeyCode::F6,
        Pc::F7 => KeyCode::F7,
        Pc::F8 => KeyCode::F8,
        Pc::F9 => KeyCode::F9,
        Pc::F10 => KeyCode::F10,
        Pc::F11 => KeyCode::F11,
        Pc::F12 => KeyCode::F12,
        Pc::PrintScreen => KeyCode::PrintScreen,
        Pc::ScrollLock => KeyCode::ScrollLock,
        Pc::PauseBreak => KeyCode::PauseBreak,
        _ => return None,
    })
}

fn from_pc_number_row(code: pc_keyboard::KeyCode) -> Option<KeyCode> {
    use pc_keyboard::KeyCode as Pc;
    Some(match code {
        Pc::Oem8 => KeyCode::Backquote,
        Pc::Key1 => KeyCode::Digit1,
        Pc::Key2 => KeyCode::Digit2,
        Pc::Key3 => KeyCode::Digit3,
        Pc::Key4 => KeyCode::Digit4,
        Pc::Key5 => KeyCode::Digit5,
        Pc::Key6 => KeyCode::Digit6,
        Pc::Key7 => KeyCode::Digit7,
        Pc::Key8 => KeyCode::Digit8,
        Pc::Key9 => KeyCode::Digit9,
        Pc::Key0 => KeyCode::Digit0,
        Pc::OemMinus => KeyCode::Minus,
        Pc::OemPlus => KeyCode::Equals,
        Pc::Backspace => KeyCode::Backspace,
        _ => return None,
    })
}

fn from_pc_letter_rows(code: pc_keyboard::KeyCode) -> Option<KeyCode> {
    use pc_keyboard::KeyCode as Pc;
    Some(match code {
        Pc::Tab => KeyCode::Tab,
        Pc::Q => KeyCode::Q,
        Pc::W => KeyCode::W,
        Pc::E => KeyCode::E,
        Pc::R => KeyCode::R,
        Pc::T => KeyCode::T,
        Pc::Y => KeyCode::Y,
        Pc::U => KeyCode::U,
        Pc::I => KeyCode::I,
        Pc::O => KeyCode::O,
        Pc::P => KeyCode::P,
        Pc::Oem4 => KeyCode::BracketLeft,
        Pc::Oem6 => KeyCode::BracketRight,
        Pc::Oem5 => KeyCode::Backslash,
        Pc::CapsLock => KeyCode::CapsLock,
        Pc::A => KeyCode::A,
        Pc::S => KeyCode::S,
        Pc::D => KeyCode::D,
        Pc::F => KeyCode::F,
        Pc::G => KeyCode::G,
        Pc::H => KeyCode::H,
        Pc::J => KeyCode::J,
        Pc::K => KeyCode::K,
        Pc::L => KeyCode::L,
        Pc::Oem1 => KeyCode::Semicolon,
        Pc::Oem3 => KeyCode::Apostrophe,
        Pc::Return => KeyCode::Return,
        Pc::LShift => KeyCode::LeftShift,
        Pc::Z => KeyCode::Z,
        Pc::X => KeyCode::X,
        Pc::C => KeyCode::C,
        Pc::V => KeyCode::V,
        Pc::B => KeyCode::B,
        Pc::N => KeyCode::N,
        Pc::M => KeyCode::M,
        Pc::OemComma => KeyCode::Comma,
        Pc::OemPeriod => KeyCode::Period,
        Pc::Oem2 => KeyCode::Slash,
        Pc::RShift => KeyCode::RightShift,
        _ => return None,
    })
}

fn from_pc_modifier_row(code: pc_keyboard::KeyCode) -> Option<KeyCode> {
    use pc_keyboard::KeyCode as Pc;
    Some(match code {
        Pc::LControl => KeyCode::LeftCtrl,
        Pc::LWin => KeyCode::LeftSuper,
        Pc::LAlt => KeyCode::LeftAlt,
        Pc::Spacebar => KeyCode::Space,
        Pc::RAltGr => KeyCode::RightAlt,
        Pc::RWin => KeyCode::RightSuper,
        Pc::Apps => KeyCode::Menu,
        Pc::RControl => KeyCode::RightCtrl,
        _ => return None,
    })
}

fn from_pc_editing_and_numpad(code: pc_keyboard::KeyCode) -> Option<KeyCode> {
    use pc_keyboard::KeyCode as Pc;
    Some(match code {
        Pc::Insert => KeyCode::Insert,
        Pc::Home => KeyCode::Home,
        Pc::PageUp => KeyCode::PageUp,
        Pc::Delete => KeyCode::Delete,
        Pc::End => KeyCode::End,
        Pc::PageDown => KeyCode::PageDown,
        Pc::ArrowUp => KeyCode::ArrowUp,
        Pc::ArrowDown => KeyCode::ArrowDown,
        Pc::ArrowLeft => KeyCode::ArrowLeft,
        Pc::ArrowRight => KeyCode::ArrowRight,
        Pc::NumpadLock => KeyCode::NumpadLock,
        Pc::NumpadDivide => KeyCode::NumpadDivide,
        Pc::NumpadMultiply => KeyCode::NumpadMultiply,
        Pc::NumpadSubtract => KeyCode::NumpadSubtract,
        Pc::NumpadAdd => KeyCode::NumpadAdd,
        Pc::NumpadEnter => KeyCode::NumpadEnter,
        Pc::NumpadPeriod => KeyCode::NumpadDecimal,
        Pc::Numpad0 => KeyCode::Numpad0,
        Pc::Numpad1 => KeyCode::Numpad1,
        Pc::Numpad2 => KeyCode::Numpad2,
        Pc::Numpad3 => KeyCode::Numpad3,
        Pc::Numpad4 => KeyCode::Numpad4,
        Pc::Numpad5 => KeyCode::Numpad5,
        Pc::Numpad6 => KeyCode::Numpad6,
        Pc::Numpad7 => KeyCode::Numpad7,
        Pc::Numpad8 => KeyCode::Numpad8,
        Pc::Numpad9 => KeyCode::Numpad9,
        _ => return None,
    })
}
