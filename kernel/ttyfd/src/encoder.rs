use roxy_input::{InputEvent, KeyCode, KeyState};

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

/// Encodes an input event into the raw bytes returned by TTY reads.
pub(crate) fn encode_input_event(event: InputEvent) -> Option<EncodedInputEvent> {
    let mut encoded = EncodedInputEvent {
        bytes: [0; 8],
        length: 0,
    };
    let bytes = match event {
        InputEvent::Character(character) => {
            let mut bytes = [0; 4];
            let length = character.encode_utf8(&mut bytes).len();
            encoded.bytes[..length].copy_from_slice(&bytes[..length]);
            encoded.length = length;

            return Some(encoded);
        }
        InputEvent::Key {
            code,
            state: KeyState::Pressed,
        } => special_key_bytes(code),
        InputEvent::Key {
            state: KeyState::Released,
            ..
        } => return None,
    };
    encoded.bytes[..bytes.len()].copy_from_slice(bytes);
    encoded.length = bytes.len();

    Some(encoded)
}

fn special_key_bytes(code: KeyCode) -> &'static [u8] {
    match code {
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
    }
}
