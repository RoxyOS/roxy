#![no_std]

extern crate alloc;

mod event;
mod mapping;

use alloc::sync::Arc;

use roxy_evdev::EvdevConfig;
use roxy_evdev_types::{BUS_I8042, EvdevCapabilities, EvdevDeviceId};
use roxy_keyboard_input::{KeyEvent, KeyboardListener};

use crate::event::EvdevEvent;

/// Stable file ID for the keyboard evdev device within the devfs mount.
const KEYBOARD_EVENT_FILE_ID: u64 = 4;

/// A wrapper around the core `roxy_evdev::EvdevDevice`.
///
/// It implements [`KeyboardListener`] for the input manager and forwards serialised events to the
/// wrapped `inner` evdev device.
pub struct EvdevKeyboard {
    inner: Arc<roxy_evdev::EvdevDevice>,
}

/// Creates the keyboard evdev device.
///
/// Returns the core devfs `Device` (for `/dev/keyboard_event`) and the keyboard `KeyboardListener`
/// (for the input manager) sharing one event queue.
///
/// # Panics
///
/// Panics when called more than once (the caller registers a single global device).
#[must_use]
pub fn create() -> (Arc<dyn roxy_devfs::Device>, Arc<EvdevKeyboard>) {
    let core = roxy_evdev::EvdevDevice::create(keyboard_config(), keyboard_capabilities());
    let keyboard = Arc::new(EvdevKeyboard {
        inner: core.clone(),
    });
    (core, keyboard)
}

/// The identity the keyboard exposes through `EVIOCGNAME`/`EVIOCGID`/etc.
fn keyboard_config() -> EvdevConfig {
    EvdevConfig {
        file_id: KEYBOARD_EVENT_FILE_ID,
        name: b"Roxy keyboard",
        phys: b"ps2/serio0/input0",
        uniq: b"",
        id: EvdevDeviceId {
            bustype: BUS_I8042,
            vendor: 0,
            product: 0,
            version: 0,
        },
    }
}

/// The event types and codes the keyboard supports (`EVIOCGBIT` answers).
fn keyboard_capabilities() -> EvdevCapabilities {
    EvdevCapabilities {
        event_types: &[roxy_evdev_types::EV_SYN, roxy_evdev_types::EV_KEY],
        key_codes: mapping::supported_key_codes(),
        led_codes: &[roxy_evdev_types::LED_CAPSL, roxy_evdev_types::LED_SCROLLL],
        switch_codes: &[],
    }
}

impl KeyboardListener for EvdevKeyboard {
    fn on_recive_input(&self, key: KeyEvent) {
        for event in event::key_state_to_evdev_pair(key.state, key.code) {
            self.push(event);
        }
    }
}

impl EvdevKeyboard {
    fn push(&self, event: EvdevEvent) {
        let record = event::encode_input_event(roxy_time::realtime_time(), event);
        self.inner.push(record);
    }
}
