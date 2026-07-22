use pc_keyboard::{DecodedKey, HandleControl, PS2Keyboard, ScancodeSet1, layouts::Us104Key};

pub(crate) struct Decoder {
    keyboard: PS2Keyboard<Us104Key, ScancodeSet1>,
}

impl Decoder {
    pub(crate) const fn new() -> Self {
        Self {
            keyboard: PS2Keyboard::new(ScancodeSet1::new(), Us104Key, HandleControl::Ignore),
        }
    }

    /// Decodes one Set 1 scan-code byte into an ASCII key press, if any.
    pub(crate) fn decode(&mut self, scancode: u8) -> Option<u8> {
        let event = match self.keyboard.add_byte(scancode) {
            Ok(Some(event)) => event,
            Ok(None) => return None,
            Err(_) => {
                self.keyboard.clear();
                return None;
            }
        };
        if matches!(event.state, pc_keyboard::KeyState::Up) {
            return None;
        }
        match self.keyboard.process_keyevent(event) {
            Some(DecodedKey::Unicode(character)) if character.is_ascii() => Some(character as u8),
            _ => None,
        }
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_test::kernel_test;

    use super::Decoder;

    fn press(decoder: &mut Decoder, scancode: u8) -> Option<u8> {
        decoder.decode(scancode)
    }

    kernel_test!("roxy-ps2::decoder-basic-keys", basic_keys, {
        let mut decoder = Decoder::new();
        assert_eq!(press(&mut decoder, 0x1e), Some(b'a'));
        assert_eq!(press(&mut decoder, 0x9e), None);
        assert_eq!(press(&mut decoder, 0x1c), Some(b'\n'));
        assert_eq!(press(&mut decoder, 0x0e), Some(8));
        assert_eq!(press(&mut decoder, 0x0f), Some(b'\t'));
        assert_eq!(press(&mut decoder, 0x39), Some(b' '));
        assert_eq!(press(&mut decoder, 0x01), Some(0x1b));
    });

    kernel_test!("roxy-ps2::decoder-shift-caps", shift_and_caps, {
        let mut decoder = Decoder::new();
        assert_eq!(press(&mut decoder, 0x2a), None);
        assert_eq!(press(&mut decoder, 0x1e), Some(b'A'));
        assert_eq!(press(&mut decoder, 0xaa), None);
        assert_eq!(press(&mut decoder, 0x3a), None);
        assert_eq!(press(&mut decoder, 0x1e), Some(b'A'));
        assert_eq!(press(&mut decoder, 0x3a), None);
        assert_eq!(press(&mut decoder, 0x1e), Some(b'a'));
    });

    kernel_test!(
        "roxy-ps2::decoder-symbols-and-extensions",
        symbols_and_extensions,
        {
            let mut decoder = Decoder::new();
            assert_eq!(press(&mut decoder, 0x02), Some(b'1'));
            assert_eq!(press(&mut decoder, 0x03), Some(b'2'));
            assert_eq!(press(&mut decoder, 0x2a), None);
            assert_eq!(press(&mut decoder, 0x02), Some(b'!'));
            assert_eq!(press(&mut decoder, 0xaa), None);
            assert_eq!(press(&mut decoder, 0xe0), None);
            assert_eq!(press(&mut decoder, 0x4b), None);
            assert_eq!(press(&mut decoder, 0xe0), None);
            assert_eq!(press(&mut decoder, 0xcb), None);
        }
    );
}
