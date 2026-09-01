use roxy_input::KeyCode;
use roxy_key_decoder::DecodedKey;

#[derive(Clone, Copy)]
pub(crate) struct EncodedInputEvent {
    bytes: [u8; 8],
    length: usize,
}

impl EncodedInputEvent {
    pub(crate) fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.length]
    }
}

/// Encodes a decoded key from the layout engine into the raw bytes
/// returned by TTY reads.
///
/// `DecodedKey::Character(char)` is encoded as its UTF-8 representation.
/// `DecodedKey::Key(code)` is encoded as a terminal escape sequence for
/// navigation and function keys; other keys produce no output.
pub(crate) fn encode_decoded(decoded: DecodedKey) -> Option<EncodedInputEvent> {
    let mut encoded = EncodedInputEvent {
        bytes: [0; 8],
        length: 0,
    };

    match decoded {
        DecodedKey::Character(character) => {
            let mut buf = [0; 4];
            let length = character.encode_utf8(&mut buf).len();
            encoded.bytes[..length].copy_from_slice(&buf[..length]);
            encoded.length = length;
            Some(encoded)
        }
        DecodedKey::Key(code) => {
            let bytes = special_key_bytes(code)?;
            encoded.bytes[..bytes.len()].copy_from_slice(bytes);
            encoded.length = bytes.len();
            Some(encoded)
        }
    }
}

/// Maps a Roxy key code to a terminal escape sequence for the 22
/// navigation and function keys that TTYs traditionally encode. All other keys
/// (including modifiers, letter keys, and the numeric keypad) return `None`.
fn special_key_bytes(code: KeyCode) -> Option<&'static [u8]> {
    Some(match code {
        KeyCode::ArrowUp => b"\x1b[A",
        KeyCode::ArrowDown => b"\x1b[B",
        KeyCode::ArrowRight => b"\x1b[C",
        KeyCode::ArrowLeft => b"\x1b[D",
        KeyCode::Home => b"\x1b[H",
        KeyCode::End => b"\x1b[F",
        KeyCode::PageUp => b"\x1b[5~",
        KeyCode::PageDown => b"\x1b[6~",
        KeyCode::Insert => b"\x1b[2~",
        KeyCode::Delete => b"\x1b[3~",
        KeyCode::F1 => b"\x1bOP",
        KeyCode::F2 => b"\x1bOQ",
        KeyCode::F3 => b"\x1bOR",
        KeyCode::F4 => b"\x1bOS",
        KeyCode::F5 => b"\x1b[15~",
        KeyCode::F6 => b"\x1b[17~",
        KeyCode::F7 => b"\x1b[18~",
        KeyCode::F8 => b"\x1b[19~",
        KeyCode::F9 => b"\x1b[20~",
        KeyCode::F10 => b"\x1b[21~",
        KeyCode::F11 => b"\x1b[23~",
        KeyCode::F12 => b"\x1b[24~",
        _ => return None,
    })
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_input::KeyCode;
    use roxy_key_decoder::DecodedKey;
    use roxy_test::kernel_test;

    use super::encode_decoded;

    kernel_test!("roxy-tty::encoder-unicode-ascii", ascii_character, {
        let encoded = encode_decoded(DecodedKey::Character('a')).unwrap();
        assert_eq!(encoded.as_bytes(), b"a");
    });

    kernel_test!("roxy-tty::encoder-unicode-multi-byte", multi_byte_utf8, {
        let encoded = encode_decoded(DecodedKey::Character('é')).unwrap();
        assert_eq!(encoded.as_bytes(), &[0xc3, 0xa9]);
    });

    kernel_test!("roxy-tty::encoder-arrow-left", arrow_left, {
        let encoded = encode_decoded(DecodedKey::Key(KeyCode::ArrowLeft)).unwrap();
        assert_eq!(encoded.as_bytes(), b"\x1b[D");
    });

    kernel_test!("roxy-tty::encoder-unknown-raw", unknown_raw_is_none, {
        assert!(encode_decoded(DecodedKey::Key(KeyCode::LeftShift)).is_none());
    });
}
