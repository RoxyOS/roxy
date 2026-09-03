#![no_std]

extern crate alloc;

use alloc::sync::Arc;

use roxy_evdev::EvdevConfig;
use roxy_evdev_types::{
    BTN_LEFT, BTN_MIDDLE, BTN_RIGHT, BUS_I8042, EV_KEY, EV_REL, EV_SYN, EvdevCapabilities,
    EvdevDeviceId, InputEvent, REL_WHEEL, REL_X, REL_Y, SYN_REPORT,
};
use roxy_mouse_input::{MouseButton, MouseEvent, MouseListener};

/// Stable file ID for the mouse evdev device within the devfs mount.
const MOUSE_EVENT_FILE_ID: u64 = 5;

/// A wrapper around the core [`roxy_evdev::EvdevDevice`] that implements
/// [`MouseListener`] and translates mouse events into the evdev input stream.
pub struct EvdevMouse {
    inner: Arc<roxy_evdev::EvdevDevice>,
}

/// Creates the mouse evdev device.
///
/// `wheel_supported` controls whether the device advertises `REL_WHEEL`; pass `true` when the
/// PS/2 mouse was detected as an `IntelliMouse` (see [`roxy_ps2::mouse_has_wheel`]).
///
/// Returns the core devfs `Device` (for `/dev/mouse_event`) and the `MouseListener` (for the
/// mouse manager) sharing one event queue.
///
/// # Panics
///
/// Panics when called more than once (the caller registers a single global device).
#[must_use]
pub fn create(wheel_supported: bool) -> (Arc<dyn roxy_devfs::Device>, Arc<EvdevMouse>) {
    let core = roxy_evdev::EvdevDevice::create(mouse_config(), mouse_capabilities(wheel_supported));
    let mouse = Arc::new(EvdevMouse {
        inner: core.clone(),
    });
    (core, mouse)
}

/// The identity the mouse exposes through `EVIOCGNAME`/`EVIOCGID`/etc.
fn mouse_config() -> EvdevConfig {
    EvdevConfig {
        file_id: MOUSE_EVENT_FILE_ID,
        name: b"Roxy mouse",
        phys: b"ps2/serio1/input0",
        uniq: b"",
        id: EvdevDeviceId {
            bustype: BUS_I8042,
            vendor: 0,
            product: 0,
            version: 0,
        },
    }
}

/// The event types and codes the mouse supports (`EVIOCGBIT` answers).
fn mouse_capabilities(wheel: bool) -> EvdevCapabilities {
    let rel_codes: &[u16] = if wheel {
        &[REL_X, REL_Y, REL_WHEEL]
    } else {
        &[REL_X, REL_Y]
    };

    EvdevCapabilities {
        event_types: &[EV_SYN, EV_KEY, EV_REL],
        key_codes: &[BTN_LEFT, BTN_RIGHT, BTN_MIDDLE],
        rel_codes,
        led_codes: &[],
        switch_codes: &[],
    }
}

impl MouseListener for EvdevMouse {
    /// Called for each hardware sample (IRQ context).  Encodes the batch of semantic events into
    /// `InputEvent` records and commits them with a single `SYN_REPORT`.
    fn on_receive_input(&self, events: &[MouseEvent]) {
        let now = roxy_time::realtime_time();

        for event in events {
            match *event {
                MouseEvent::Move { right, down } => {
                    self.push(now, EV_REL, REL_X, right);
                    self.push(now, EV_REL, REL_Y, down);
                }
                MouseEvent::Scroll { up } => {
                    self.push(now, EV_REL, REL_WHEEL, up);
                }
                MouseEvent::ButtonPressed(button) => {
                    self.push(now, EV_KEY, button_to_evdev(button), 1);
                }
                MouseEvent::ButtonReleased(button) => {
                    self.push(now, EV_KEY, button_to_evdev(button), 0);
                }
            }
        }

        self.push(now, EV_SYN, SYN_REPORT, 0);
    }
}

impl EvdevMouse {
    fn push(&self, now: core::time::Duration, type_: u16, code: u16, value: i32) {
        let record = InputEvent {
            tv_sec: i64::try_from(now.as_secs()).expect("realtime seconds fit in i64"),
            tv_usec: i64::from(now.subsec_micros()),
            type_,
            code,
            value,
        };
        self.inner.push(record);
    }
}

/// Maps a semantic [`MouseButton`] to the Linux evdev `BTN_*` code.
fn button_to_evdev(button: MouseButton) -> u16 {
    match button {
        MouseButton::Left => BTN_LEFT,
        MouseButton::Right => BTN_RIGHT,
        MouseButton::Middle => BTN_MIDDLE,
    }
}
