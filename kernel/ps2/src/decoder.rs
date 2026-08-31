use pc_keyboard::{DecodedKey, HandleControl, PS2Keyboard, ScancodeSet1, layouts::Us104Key};
use roxy_input::{InputEvent, KeyCode, KeyState};

pub(crate) struct Decoder {
    keyboard: PS2Keyboard<Us104Key, ScancodeSet1>,
}

impl Decoder {
    pub(crate) const fn new() -> Self {
        Self {
            keyboard: PS2Keyboard::new(
                ScancodeSet1::new(),
                Us104Key,
                HandleControl::MapLettersToUnicode,
            ),
        }
    }

    /// Decodes one Set 1 scan-code byte into an input event, if any.
    pub(crate) fn decode(&mut self, scancode: u8) -> Option<InputEvent> {
        let event = match self.keyboard.add_byte(scancode) {
            Ok(Some(event)) => event,
            Ok(None) => return None,
            Err(_) => {
                self.keyboard.clear();
                return None;
            }
        };

        let state = match event.state {
            pc_keyboard::KeyState::Up => KeyState::Released,
            pc_keyboard::KeyState::Down | pc_keyboard::KeyState::SingleShot => KeyState::Pressed,
        };

        match self.keyboard.process_keyevent(event) {
            Some(DecodedKey::Unicode(character)) if state == KeyState::Pressed => {
                Some(InputEvent::Character(character))
            }
            Some(DecodedKey::RawKey(code)) => {
                map_key(code).map(|code| InputEvent::Key { code, state })
            }
            _ => None,
        }
    }
}

fn map_key(code: pc_keyboard::KeyCode) -> Option<KeyCode> {
    Some(match code {
        pc_keyboard::KeyCode::ArrowUp => KeyCode::ArrowUp,
        pc_keyboard::KeyCode::ArrowDown => KeyCode::ArrowDown,
        pc_keyboard::KeyCode::ArrowLeft => KeyCode::ArrowLeft,
        pc_keyboard::KeyCode::ArrowRight => KeyCode::ArrowRight,
        pc_keyboard::KeyCode::Home => KeyCode::Home,
        pc_keyboard::KeyCode::End => KeyCode::End,
        pc_keyboard::KeyCode::PageUp => KeyCode::PageUp,
        pc_keyboard::KeyCode::PageDown => KeyCode::PageDown,
        pc_keyboard::KeyCode::Insert => KeyCode::Insert,
        pc_keyboard::KeyCode::Delete => KeyCode::Delete,
        pc_keyboard::KeyCode::F1 => KeyCode::F1,
        pc_keyboard::KeyCode::F2 => KeyCode::F2,
        pc_keyboard::KeyCode::F3 => KeyCode::F3,
        pc_keyboard::KeyCode::F4 => KeyCode::F4,
        pc_keyboard::KeyCode::F5 => KeyCode::F5,
        pc_keyboard::KeyCode::F6 => KeyCode::F6,
        pc_keyboard::KeyCode::F7 => KeyCode::F7,
        pc_keyboard::KeyCode::F8 => KeyCode::F8,
        pc_keyboard::KeyCode::F9 => KeyCode::F9,
        pc_keyboard::KeyCode::F10 => KeyCode::F10,
        pc_keyboard::KeyCode::F11 => KeyCode::F11,
        pc_keyboard::KeyCode::F12 => KeyCode::F12,
        _ => return None,
    })
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_input::{InputEvent, KeyCode, KeyState};
    use roxy_test::kernel_test;

    use super::Decoder;

    fn press(decoder: &mut Decoder, scancode: u8) -> Option<InputEvent> {
        decoder.decode(scancode)
    }

    kernel_test!("roxy-ps2::decoder-basic-keys", basic_keys, {
        let mut decoder = Decoder::new();
        assert_eq!(press(&mut decoder, 0x1e), Some(InputEvent::Character('a')));
        assert_eq!(press(&mut decoder, 0x9e), None);
        assert_eq!(press(&mut decoder, 0x1c), Some(InputEvent::Character('\n')));
        assert_eq!(
            press(&mut decoder, 0x0e),
            Some(InputEvent::Character('\u{8}'))
        );
        assert_eq!(press(&mut decoder, 0x0f), Some(InputEvent::Character('\t')));
        assert_eq!(press(&mut decoder, 0x39), Some(InputEvent::Character(' ')));
        assert_eq!(
            press(&mut decoder, 0x01),
            Some(InputEvent::Character('\u{1b}'))
        );
    });

    kernel_test!("roxy-ps2::decoder-shift-caps", shift_and_caps, {
        let mut decoder = Decoder::new();
        assert_eq!(press(&mut decoder, 0x2a), None);
        assert_eq!(press(&mut decoder, 0x1e), Some(InputEvent::Character('A')));
        assert_eq!(press(&mut decoder, 0xaa), None);
        assert_eq!(press(&mut decoder, 0x3a), None);
        assert_eq!(press(&mut decoder, 0x1e), Some(InputEvent::Character('A')));
        assert_eq!(press(&mut decoder, 0x3a), None);
        assert_eq!(press(&mut decoder, 0x1e), Some(InputEvent::Character('a')));
    });

    kernel_test!(
        "roxy-ps2::decoder-ctrl",
        ctrl_translates_letters_to_control_characters,
        {
            let mut decoder = Decoder::new();
            // LCtrl down (0x1d), then C (0x2e): Ctrl+C becomes 0x03.
            assert_eq!(press(&mut decoder, 0x1d), None);
            assert_eq!(
                press(&mut decoder, 0x2e),
                Some(InputEvent::Character('\u{3}'))
            );
            // LCtrl up (0x9d); plain C now returns lowercase 'c'.
            assert_eq!(press(&mut decoder, 0x9d), None);
            assert_eq!(press(&mut decoder, 0x2e), Some(InputEvent::Character('c')));
        }
    );

    kernel_test!(
        "roxy-ps2::decoder-symbols-and-extensions",
        symbols_and_extensions,
        {
            let mut decoder = Decoder::new();
            assert_eq!(press(&mut decoder, 0x02), Some(InputEvent::Character('1')));
            assert_eq!(press(&mut decoder, 0x03), Some(InputEvent::Character('2')));
            assert_eq!(press(&mut decoder, 0x2a), None);
            assert_eq!(press(&mut decoder, 0x02), Some(InputEvent::Character('!')));
            assert_eq!(press(&mut decoder, 0xaa), None);
            assert_eq!(press(&mut decoder, 0xe0), None);
            assert_eq!(
                press(&mut decoder, 0x4b),
                Some(InputEvent::Key {
                    code: KeyCode::ArrowLeft,
                    state: KeyState::Pressed,
                })
            );
            assert_eq!(press(&mut decoder, 0xe0), None);
            assert_eq!(
                press(&mut decoder, 0xcb),
                Some(InputEvent::Key {
                    code: KeyCode::ArrowLeft,
                    state: KeyState::Released,
                })
            );
        }
    );
}
