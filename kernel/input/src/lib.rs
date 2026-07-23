#![no_std]

/// A shared source of decoded input events.
pub trait InputDevice: Send + Sync {
    /// Returns the oldest available event without blocking.
    #[must_use]
    fn read_event(&self) -> Option<InputEvent>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputEvent {
    Character(char),
    Key { code: KeyCode, state: KeyState },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyState {
    Pressed,
    Released,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyCode {
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    Delete,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
}
