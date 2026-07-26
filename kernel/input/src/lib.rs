#![no_std]

extern crate alloc;

use alloc::{
    sync::{Arc, Weak},
    vec::Vec,
};

use roxy_utils::Lock;

/// A shared source of decoded input events.
pub trait InputDevice: Send + Sync {
    /// Returns the oldest available event without blocking.
    #[must_use]
    fn read_event(&self) -> Option<InputEvent>;

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
