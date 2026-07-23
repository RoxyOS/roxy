use heapless::Deque;
use roxy_input::InputEvent;

use crate::decoder::Decoder;

const INPUT_CAPACITY: usize = 256;

/// Owns scan-code decoding and the bounded queue of input events.
pub(crate) struct KeyboardInput {
    decoder: Decoder,
    events: Deque<InputEvent, INPUT_CAPACITY>,
}

impl KeyboardInput {
    pub(crate) const fn new() -> Self {
        Self {
            decoder: Decoder::new(),
            events: Deque::new(),
        }
    }

    pub(crate) fn process_scancode(&mut self, scancode: u8) {
        let Some(event) = self.decoder.decode(scancode) else {
            return;
        };
        self.enqueue_event(event);
    }

    pub(crate) fn read(&mut self) -> Option<InputEvent> {
        self.events.pop_front()
    }

    pub(crate) fn enqueue_event(&mut self, event: InputEvent) {
        let _ = self.events.push_back(event);
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_input::InputEvent;
    use roxy_test::kernel_test;

    use super::{INPUT_CAPACITY, KeyboardInput};

    kernel_test!("roxy-ps2::queue-order-and-drop", queue_behavior, {
        let mut input = KeyboardInput::new();
        assert_eq!(input.read(), None);

        for value in 0..INPUT_CAPACITY {
            input.enqueue_event(InputEvent::Character(
                char::from_u32(u32::try_from(value).unwrap()).unwrap(),
            ));
        }
        input.enqueue_event(InputEvent::Character('\0'));
        assert_eq!(input.read(), Some(InputEvent::Character('\0')));
        assert_eq!(input.read(), Some(InputEvent::Character('\u{1}')));
        assert_eq!(input.read(), Some(InputEvent::Character('\u{2}')));
        assert_eq!(input.read(), Some(InputEvent::Character('\u{3}')));
    });
}
