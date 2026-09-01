#![no_std]

extern crate alloc;

use alloc::{
    sync::{Arc, Weak},
    vec::Vec,
};

use roxy_utils::Lock;

/// A shared source of raw keyboard events.
///
/// Implementations expose physical key presses and releases without applying a keyboard layout:
/// layout mapping (scancode to character, modifiers, dead keys) is owned by consumers. The TTY
/// decodes events through `pc_keyboard`; a future graphics stack maps them through its own layout
/// engine (e.g. XKB).
pub trait InputDevice: Send + Sync {
    /// Returns the oldest available key event without blocking.
    #[must_use]
    fn read_key(&self) -> Option<KeyEvent>;

    /// Registers a listener notified after this device receives input.
    fn register_listener(&self, _listener: Arc<dyn InputListener>) {}
}

/// Receives notification that an input device may have queued another event.
pub trait InputListener: Send + Sync {
    fn on_recive_input(&self);
}

/// Owns listeners notified when an input device queues an event.
pub struct InputListeners {
    listeners: Lock<Vec<Weak<dyn InputListener>>>,
}

impl InputListeners {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            listeners: Lock::new(Vec::new()),
        }
    }

    pub fn register(&self, listener: &Arc<dyn InputListener>) {
        self.listeners.lock().push(Arc::downgrade(listener));
    }

    pub fn notify(&self) {
        let mut listeners = self.listeners.lock();
        listeners.retain(|listener| {
            let Some(listener) = listener.upgrade() else {
                return false;
            };

            listener.on_recive_input();
            true
        });
    }
}

impl Default for InputListeners {
    fn default() -> Self {
        Self::new()
    }
}

/// One physical key press or release.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KeyEvent {
    pub code: KeyCode,
    pub state: KeyState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyState {
    Pressed,
    Released,
}

/// A physical keyboard key. The set covers a US 104-key keyboard and is layout-neutral: the
/// enum is the single source of truth for key identity, while layout mapping happens in
/// consumers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyCode {
    // Row of function keys and system keys.
    Escape,
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
    PrintScreen,
    ScrollLock,
    PauseBreak,

    // Number row.
    Backquote,
    Digit1,
    Digit2,
    Digit3,
    Digit4,
    Digit5,
    Digit6,
    Digit7,
    Digit8,
    Digit9,
    Digit0,
    Minus,
    Equals,
    Backspace,

    // Top letter row.
    Tab,
    Q,
    W,
    E,
    R,
    T,
    Y,
    U,
    I,
    O,
    P,
    BracketLeft,
    BracketRight,
    Backslash,

    // Home/middle letter row.
    CapsLock,
    A,
    S,
    D,
    F,
    G,
    H,
    J,
    K,
    L,
    Semicolon,
    Apostrophe,

    // Bottom letter row.
    LeftShift,
    Z,
    X,
    C,
    V,
    B,
    N,
    M,
    Comma,
    Period,
    Slash,
    RightShift,

    // Bottom modifier row.
    LeftCtrl,
    LeftSuper,
    LeftAlt,
    Space,
    RightAlt,
    RightSuper,
    Menu,
    RightCtrl,

    // Editing block.
    Insert,
    Home,
    PageUp,
    Delete,
    End,
    PageDown,
    Return,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,

    // Numeric keypad.
    NumpadLock,
    NumpadDivide,
    NumpadMultiply,
    NumpadSubtract,
    NumpadAdd,
    NumpadEnter,
    NumpadDecimal,
    Numpad0,
    Numpad1,
    Numpad2,
    Numpad3,
    Numpad4,
    Numpad5,
    Numpad6,
    Numpad7,
    Numpad8,
    Numpad9,
}
