use pc_keyboard::{ScancodeSet, ScancodeSet1};
use roxy_keyboard_input::{KeyCode, KeyEvent, KeyState};

pub(crate) struct ScancodeParser {
    set: ScancodeSet1,
}

impl ScancodeParser {
    pub(crate) const fn new() -> Self {
        Self {
            set: ScancodeSet1::new(),
        }
    }

    /// Parses one Set 1 scan-code byte into a physical key event, if any.
    ///
    /// This is a pure scan-code parse: no keyboard layout, modifier, or character decoding is
    /// applied. Every physical key press and release (including modifiers and character-key
    /// releases) becomes a `KeyEvent`, leaving layout mapping to consumers.
    pub(crate) fn parse(&mut self, scancode: u8) -> Option<KeyEvent> {
        let Ok(Some(event)) = self.set.advance_state(scancode) else {
            return None;
        };

        let state = match event.state {
            pc_keyboard::KeyState::Up => KeyState::Released,
            pc_keyboard::KeyState::Down | pc_keyboard::KeyState::SingleShot => KeyState::Pressed,
        };

        map_key(event.code).map(|code| KeyEvent { code, state })
    }
}

/// Maps a pc-keyboard physical key to the layout-neutral Roxy key code.
///
/// Keys that are not present on a US 104-key keyboard (ISO-only keys, media keys, protocol
/// states) map to `None`.
fn map_key(code: pc_keyboard::KeyCode) -> Option<KeyCode> {
    map_function_row(code)
        .or_else(|| map_number_row(code))
        .or_else(|| map_letter_rows(code))
        .or_else(|| map_modifier_row(code))
        .or_else(|| map_editing_and_numpad(code))
}

fn map_function_row(code: pc_keyboard::KeyCode) -> Option<KeyCode> {
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

fn map_number_row(code: pc_keyboard::KeyCode) -> Option<KeyCode> {
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

fn map_letter_rows(code: pc_keyboard::KeyCode) -> Option<KeyCode> {
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

fn map_modifier_row(code: pc_keyboard::KeyCode) -> Option<KeyCode> {
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

fn map_editing_and_numpad(code: pc_keyboard::KeyCode) -> Option<KeyCode> {
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

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_keyboard_input::{KeyCode, KeyEvent, KeyState};
    use roxy_test::kernel_test;

    use super::ScancodeParser;

    fn press(decoder: &mut ScancodeParser, scancode: u8) -> Option<KeyEvent> {
        decoder.parse(scancode)
    }

    fn event(code: KeyCode, state: KeyState) -> KeyEvent {
        KeyEvent { code, state }
    }

    kernel_test!("roxy-ps2::scancode-basic-keys", basic_keys, {
        let mut decoder = ScancodeParser::new();
        assert_eq!(
            press(&mut decoder, 0x1e),
            Some(event(KeyCode::A, KeyState::Pressed))
        );
        assert_eq!(
            press(&mut decoder, 0x9e),
            Some(event(KeyCode::A, KeyState::Released))
        );
        assert_eq!(
            press(&mut decoder, 0x1c),
            Some(event(KeyCode::Return, KeyState::Pressed))
        );
        assert_eq!(
            press(&mut decoder, 0x0e),
            Some(event(KeyCode::Backspace, KeyState::Pressed))
        );
        assert_eq!(
            press(&mut decoder, 0x0f),
            Some(event(KeyCode::Tab, KeyState::Pressed))
        );
        assert_eq!(
            press(&mut decoder, 0x39),
            Some(event(KeyCode::Space, KeyState::Pressed))
        );
        assert_eq!(
            press(&mut decoder, 0x01),
            Some(event(KeyCode::Escape, KeyState::Pressed))
        );
    });

    kernel_test!(
        "roxy-ps2::scancode-modifiers",
        modifiers_press_and_release,
        {
            let mut decoder = ScancodeParser::new();
            assert_eq!(
                press(&mut decoder, 0x2a),
                Some(event(KeyCode::LeftShift, KeyState::Pressed))
            );
            assert_eq!(
                press(&mut decoder, 0xaa),
                Some(event(KeyCode::LeftShift, KeyState::Released))
            );
            assert_eq!(
                press(&mut decoder, 0x3a),
                Some(event(KeyCode::CapsLock, KeyState::Pressed))
            );
            assert_eq!(
                press(&mut decoder, 0x1d),
                Some(event(KeyCode::LeftCtrl, KeyState::Pressed))
            );
            assert_eq!(
                press(&mut decoder, 0x9d),
                Some(event(KeyCode::LeftCtrl, KeyState::Released))
            );
        }
    );

    kernel_test!("roxy-ps2::scancode-numbers-and-symbols", number_row, {
        let mut decoder = ScancodeParser::new();
        assert_eq!(
            press(&mut decoder, 0x02),
            Some(event(KeyCode::Digit1, KeyState::Pressed))
        );
        assert_eq!(
            press(&mut decoder, 0x03),
            Some(event(KeyCode::Digit2, KeyState::Pressed))
        );
        // 0x0c is the minus key in Set 1.
        assert_eq!(
            press(&mut decoder, 0x0c),
            Some(event(KeyCode::Minus, KeyState::Pressed))
        );
    });

    kernel_test!("roxy-ps2::scancode-extended-keys", extended_keys, {
        let mut decoder = ScancodeParser::new();
        // 0xe0 prefixes an extended key; the next byte completes the sequence.
        assert_eq!(press(&mut decoder, 0xe0), None);
        assert_eq!(
            press(&mut decoder, 0x4b),
            Some(event(KeyCode::ArrowLeft, KeyState::Pressed))
        );
        assert_eq!(press(&mut decoder, 0xe0), None);
        assert_eq!(
            press(&mut decoder, 0xcb),
            Some(event(KeyCode::ArrowLeft, KeyState::Released))
        );
    });
}
