use heapless::Deque;
use roxy_input::KeyEvent;

use crate::scancode::ScancodeParser;

const INPUT_CAPACITY: usize = 256;

/// Owns scan-code parsing and the bounded queue of raw key events.
pub(crate) struct KeyboardInput {
    scancode_parser: ScancodeParser,
    events: Deque<KeyEvent, INPUT_CAPACITY>,
}

impl KeyboardInput {
    pub(crate) const fn new() -> Self {
        Self {
            scancode_parser: ScancodeParser::new(),
            events: Deque::new(),
        }
    }

    pub(crate) fn process_scancode(&mut self, scancode: u8) -> Result<(), ()> {
        let Some(event) = self.scancode_parser.parse(scancode) else {
            return Err(());
        };

        self.enqueue_event(event)
    }

    pub(crate) fn read(&mut self) -> Option<KeyEvent> {
        self.events.pop_front()
    }

    pub(crate) fn enqueue_event(&mut self, event: KeyEvent) -> Result<(), ()> {
        self.events.push_back(event).map_err(|_| ())
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_input::{KeyCode, KeyEvent, KeyState};
    use roxy_test::kernel_test;

    use super::{INPUT_CAPACITY, KeyboardInput};

    fn event(index: usize) -> KeyEvent {
        KeyEvent {
            code: KeyCode::Digit0,
            state: if index.is_multiple_of(2) {
                KeyState::Pressed
            } else {
                KeyState::Released
            },
        }
    }

    kernel_test!("roxy-ps2::queue-order-and-drop", queue_behavior, {
        let mut input = KeyboardInput::new();
        assert_eq!(input.read(), None);

        for index in 0..INPUT_CAPACITY {
            input.enqueue_event(event(index)).unwrap();
        }

        let _ = input.enqueue_event(event(INPUT_CAPACITY));
        assert_eq!(input.read(), Some(event(0)));
        assert_eq!(input.read(), Some(event(1)));
        assert_eq!(input.read(), Some(event(2)));
        assert_eq!(input.read(), Some(event(3)));
    });
}
