#![no_std]

extern crate alloc;

use alloc::{
    sync::{Arc, Weak},
    vec::Vec,
};

use roxy_utils::Lock;

/// Receives every parsed key event published to the keyboard manager.
pub trait KeyboardListener: Send + Sync {
    /// Called once per parsed key event, in the producer's context (IRQ).
    fn on_recive_input(&self, key: KeyEvent);
}

/// Owns the listener registry and broadcasts parsed key events to every registered listener.
///
/// The manager is a global singleton: drivers publish events via [`publish`], and consumers
/// register via [`register_listener`]. Registration happens at boot (before interrupts are
/// enabled), after which `publish` is called only from IRQ context.
pub struct KeyboardManager {
    listeners: Lock<Vec<Weak<dyn KeyboardListener>>>,
}

static KEYBOARD_MANAGER: KeyboardManager = KeyboardManager::new();

impl Default for KeyboardManager {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyboardManager {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            listeners: Lock::new(Vec::new()),
        }
    }

    /// Registers a listener to receive every future key event.
    ///
    /// The manager stores a weak reference; the caller must keep the `Arc` alive for the
    /// listener to remain registered.
    pub fn register(&self, listener: &Arc<dyn KeyboardListener>) {
        self.listeners.lock().push(Arc::downgrade(listener));
    }

    /// Broadcasts one key event to every registered listener.
    ///
    /// Called from the driver's IRQ handler. Each listener receives a copy of the event.
    /// Expired listeners (whose `Arc` was dropped) are cleaned up during the iteration.
    pub fn publish(&self, key: KeyEvent) {
        let mut listeners = self.listeners.lock();
        listeners.retain(|listener| {
            let Some(listener) = listener.upgrade() else {
                return false;
            };
            listener.on_recive_input(key);
            true
        });
    }
}

/// Registers a listener with the process-wide keyboard manager.
///
/// Called at boot for each consumer (TTY, keyboard evdev, etc.).
pub fn register_listener(listener: &Arc<dyn KeyboardListener>) {
    KEYBOARD_MANAGER.register(listener);
}

/// Publishes one parsed key event to every registered listener.
///
/// Called by the keyboard driver's IRQ handler.
pub fn publish(key: KeyEvent) {
    KEYBOARD_MANAGER.publish(key);
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
