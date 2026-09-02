//! Keyboard-specific evdev event types.
//!
//! [`EvdevEvent`] is a semantic enum (carrying `KeyCode` rather than raw `KEY_*` integers) that
//! the keyboard device feeds to [`encode_input_event`] to produce a serialised [`InputEvent`].

use roxy_evdev_types::{EV_KEY, EV_SYN, InputEvent, SYN_REPORT};
use roxy_input::{KeyCode, KeyState};

/// A semantic evdev event that the keyboard device can produce.
///
/// The enum carries `KeyCode` (a keyboard concept) and is converted to the raw `InputEvent`
/// wire record only at serialisation time, via [`encode_input_event`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EvdevEvent {
    /// A key was pressed (`EV_KEY`, value = 1).
    KeyPressed(KeyCode),
    /// A key was released (`EV_KEY`, value = 0).
    KeyReleased(KeyCode),
    /// End of an event batch (`EV_SYN`, `SYN_REPORT`).
    SynReport,
}

/// Builds the two evdev events for one key transition: the `(EV_KEY, code, value)` event plus
/// the `(EV_SYN, SYN_REPORT, 0)` commit that terminates the batch.
#[must_use]
pub fn key_state_to_evdev_pair(state: KeyState, key: KeyCode) -> [EvdevEvent; 2] {
    let key_event = match state {
        KeyState::Pressed => EvdevEvent::KeyPressed(key),
        KeyState::Released => EvdevEvent::KeyReleased(key),
    };

    [key_event, EvdevEvent::SynReport]
}

/// Encodes one keyboard `EvdevEvent` into an `InputEvent`, filling the timestamp from the
/// realtime clock.
///
/// # Panics
///
/// Panics if a `KeyPressed`/`KeyReleased` code has no `KEY_*` mapping (the 104-key set is
/// fully covered).
#[must_use]
pub fn encode_input_event(now: core::time::Duration, event: EvdevEvent) -> InputEvent {
    let (type_, code, value) = match event {
        EvdevEvent::KeyPressed(key) => (EV_KEY, crate::mapping::keycode_to_evdev(key), 1),
        EvdevEvent::KeyReleased(key) => (EV_KEY, crate::mapping::keycode_to_evdev(key), 0),
        EvdevEvent::SynReport => (EV_SYN, SYN_REPORT, 0),
    };

    InputEvent {
        tv_sec: i64::try_from(now.as_secs()).expect("realtime seconds fit in i64"),
        tv_usec: i64::from(now.subsec_micros()),
        type_,
        code,
        value,
    }
}
