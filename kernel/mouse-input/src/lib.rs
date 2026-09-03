#![no_std]

extern crate alloc;

use alloc::{
    sync::{Arc, Weak},
    vec::Vec,
};

use roxy_utils::Lock;

/// A physical mouse button.
///
/// The set covers the buttons the current PS/2 mouse protocols can report (left, right,
/// middle).  Fourth/fifth buttons (`BTN_SIDE`/`BTN_EXTRA`) belong to future devices and are
/// added when a driver can produce them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// A semantic mouse input event.
///
/// Each variant represents one atomic occurrence.  A single hardware sample (e.g. one PS/2
/// packet) may produce several events: a `Move`, a `Scroll`, and zero or more
/// `ButtonPressed`/`ButtonReleased` for buttons whose state changed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MouseEvent {
    /// Pointer relative motion.  `right` is positive to the right, `down` is positive
    /// downward.  Units are hardware counts (DPI-dependent).
    Move { right: i32, down: i32 },
    /// Wheel scroll.  `up` is positive for upward scrolling.
    Scroll { up: i32 },
    /// A button was pressed.
    ButtonPressed(MouseButton),
    /// A button was released.
    ButtonReleased(MouseButton),
}

/// Receives mouse input events in the producer's context (IRQ).
///
/// Each call delivers a batch of events that belongs to one hardware sample.  The listener
/// may buffer or process the events immediately; it must not block or allocate.
pub trait MouseListener: Send + Sync {
    /// Called once per hardware sample (IRQ context).
    fn on_receive_input(&self, events: &[MouseEvent]);
}

/// Owns the listener registry and broadcasts mouse events to every registered listener.
///
/// The manager is a global singleton: drivers publish events via [`publish`], and consumers
/// register via [`register_listener`].  Registration happens at boot (before interrupts are
/// enabled), after which `publish` is called only from IRQ context.
pub struct MouseManager {
    listeners: Lock<Vec<Weak<dyn MouseListener>>>,
}

static MOUSE_MANAGER: MouseManager = MouseManager::new();

impl Default for MouseManager {
    fn default() -> Self {
        Self::new()
    }
}

impl MouseManager {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            listeners: Lock::new(Vec::new()),
        }
    }

    /// Registers a listener to receive every future mouse event batch.
    ///
    /// The manager stores a weak reference; the caller must keep the `Arc` alive for the
    /// listener to remain registered.
    pub fn register(&self, listener: &Arc<dyn MouseListener>) {
        self.listeners.lock().push(Arc::downgrade(listener));
    }

    /// Broadcasts one batch of mouse events to every registered listener.
    ///
    /// Called from the driver's IRQ handler.  Expired listeners (whose `Arc` was dropped) are
    /// cleaned up during the iteration.
    pub fn publish(&self, events: &[MouseEvent]) {
        let mut listeners = self.listeners.lock();
        listeners.retain(|listener| {
            let Some(listener) = listener.upgrade() else {
                return false;
            };
            listener.on_receive_input(events);
            true
        });
    }
}

/// Registers a listener with the process-wide mouse manager.
///
/// Called at boot for each consumer (mouse evdev, future TTY mouse support, …).
pub fn register_listener(listener: &Arc<dyn MouseListener>) {
    MOUSE_MANAGER.register(listener);
}

/// Publishes one batch of mouse events to every registered listener.
///
/// Called by the mouse driver's IRQ handler.
pub fn publish(events: &[MouseEvent]) {
    MOUSE_MANAGER.publish(events);
}
