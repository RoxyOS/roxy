//! Decodes a PS/2 mouse byte stream into semantic [`MouseEvent`] batches.
//!
//! Protocol reference: `OSDev` Wiki "PS/2 Mouse" (packet bit layout, 9-bit sign extension,
//! `IntelliMouse` Z-axis extension); Linux `drivers/input/mouse/psmouse-base.c`
//! (`psmouse_report_standard_motion`, `psmouse_report_standard_buttons`, and the `PSMOUSE_IMPS`
//! wheel decode) as the behavioral cross-check.
//!
//! Decoding mirrors Linux's standard PS/2 handling:
//!
//! - X and Y are 9-bit signed deltas.  The 8-bit delta byte is combined with the sign bit in
//!   the packet's first byte:
//!   `x = byte1 - ((byte0 << 4) & 0x100)`, `y = byte2 - ((byte0 << 3) & 0x100)`; a zero delta
//!   byte yields zero motion (Linux guards the sign extension with a non-zero check).
//! - Linux negates the Y delta before reporting it (`input_report_rel(dev, REL_Y, -y)`), so a
//!   hardware-positive Y (moving "up") becomes a screen-negative value.  Our [`MouseEvent::Move`]
//!   `down` field is positive downward, matching that screen convention.
//! - The `IntelliMouse` Z-axis byte is sign-extended and negated (`-(s8)packet[3]`); positive
//!   [`MouseEvent::Scroll`] `up` means scrolling upward.
//! - Buttons are the low three bits of byte0 (bit0 left, bit1 right, bit2 middle).  A
//!   [`MouseEvent::ButtonPressed`]/[`MouseEvent::ButtonReleased`] is emitted only when a
//!   button's state changes between packets.
//!
//! Overflow bits (`yo`/`xo`) are ignored, matching Linux's standard motion report.

use heapless::Vec;
use roxy_mouse_input::{MouseButton, MouseEvent};

/// Maximum events one packet can produce: up to three button transitions plus one move plus
/// one scroll.
const MAX_EVENTS_PER_PACKET: usize = 5;

/// Bit 3 of the first packet byte is always 1, used to resynchronize the stream.
const SYNC_BIT: u8 = 0x08;
const BUTTON_BITS: u8 = 0x07;

/// The negotiated PS/2 mouse packet format.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PacketMode {
    /// Standard 3-byte packets (no wheel).
    Standard,
    /// `IntelliMouse` 4-byte packets (adds a Z-axis wheel byte).
    Intellimouse,
}

impl PacketMode {
    pub(crate) fn packet_len(self) -> usize {
        match self {
            Self::Standard => 3,
            Self::Intellimouse => 4,
        }
    }
}

/// Stateful decoder that accumulates bytes and emits one batch of events per full packet.
pub(crate) struct MousePacketParser {
    mode: PacketMode,
    buffer: [u8; 4],
    count: usize,
    previous_buttons: u8,
}

impl MousePacketParser {
    pub(crate) const fn new() -> Self {
        Self {
            mode: PacketMode::Standard,
            buffer: [0; 4],
            count: 0,
            previous_buttons: 0,
        }
    }

    /// Selects the packet length for the detected mouse protocol.
    pub(crate) fn set_mode(&mut self, mode: PacketMode) {
        self.mode = mode;
        self.reset();
    }

    fn reset(&mut self) {
        self.count = 0;
        self.previous_buttons = 0;
    }

    /// Feeds one byte from IRQ12 and returns a full packet's events, if a packet completed.
    pub(crate) fn feed(&mut self, byte: u8) -> Option<Vec<MouseEvent, MAX_EVENTS_PER_PACKET>> {
        if self.count == 0 {
            // A packet's first byte must have bit 3 set; discard stray bytes to resync.
            if byte & SYNC_BIT == 0 {
                return None;
            }
            self.buffer[0] = byte;
            self.count = 1;
            return None;
        }

        self.buffer[self.count] = byte;
        self.count += 1;
        if self.count < self.mode.packet_len() {
            return None;
        }
        self.count = 0;
        Some(self.decode())
    }

    fn decode(&mut self) -> Vec<MouseEvent, MAX_EVENTS_PER_PACKET> {
        let mut events = Vec::new();
        let b0 = self.buffer[0];

        for (bit, button) in [(1, MouseButton::Right), (2, MouseButton::Middle)] {
            self.emit_button(&mut events, b0, bit, button);
        }
        self.emit_button(&mut events, b0, 0, MouseButton::Left);
        self.previous_buttons = b0 & BUTTON_BITS;

        let x = delta(self.buffer[1], b0, 4);
        let y = delta(self.buffer[2], b0, 3);
        if x != 0 || y != 0 {
            let _ = events.push(MouseEvent::Move { right: x, down: -y });
        }

        if self.mode == PacketMode::Intellimouse {
            // SAFETY: IntelliMouse Z byte is two's-complement signed; converting u8 → i8 (which
            // may wrap the value representation) then negating is the documented Linux behaviour
            // (`-(s8)packet[3]` in psmouse-base.c).
            #[allow(clippy::cast_possible_wrap)]
            let wheel = -i32::from(self.buffer[3] as i8);
            if wheel != 0 {
                let _ = events.push(MouseEvent::Scroll { up: wheel });
            }
        }

        events
    }

    fn emit_button(
        &mut self,
        events: &mut Vec<MouseEvent, MAX_EVENTS_PER_PACKET>,
        b0: u8,
        bit: u8,
        button: MouseButton,
    ) {
        let mask = 1 << bit;
        let pressed = b0 & mask != 0;
        let was_pressed = self.previous_buttons & mask != 0;
        if pressed != was_pressed {
            let _ = events.push(if pressed {
                MouseEvent::ButtonPressed(button)
            } else {
                MouseEvent::ButtonReleased(button)
            });
        }
    }
}

/// Computes the 9-bit signed delta for one axis, mirroring Linux's standard motion report.
///
/// `shift` is the sign-bit position within `b0` shifted to bit 8 (`4` for X, `3` for Y).  A
/// zero delta byte produces zero motion regardless of the sign bit.
fn delta(byte: u8, b0: u8, shift: u8) -> i32 {
    if byte == 0 {
        0
    } else {
        // Subtract 0x100 when the sign bit (shifted to bit 8) is set, forming the 9-bit
        // signed delta.
        i32::from(byte) - ((i32::from(b0) << shift) & 0x100)
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use heapless::Vec;
    use roxy_mouse_input::{MouseButton, MouseEvent};
    use roxy_test::kernel_test;

    use super::{MousePacketParser, PacketMode};

    fn events(bytes: &[u8]) -> Vec<MouseEvent, 5> {
        let mut parser = MousePacketParser::new();
        let mut output = Vec::new();
        for &byte in bytes {
            if let Some(batch) = parser.feed(byte) {
                let _ = output.extend_from_slice(&batch);
            }
        }
        output
    }

    kernel_test!("roxy-ps2::mouse-packet-motion", decodes_motion, {
        // byte0 = 0x08 (sync bit, no buttons), x=+2, y=+3.
        let events = events(&[0x08, 0x02, 0x03]);
        // Linux negates Y, so screen `down` is -3 (moved up).
        assert_eq!(events, [MouseEvent::Move { right: 2, down: -3 }]);
    });

    kernel_test!(
        "roxy-ps2::mouse-packet-negative-motion",
        decodes_negative,
        {
            // xs (bit4) set -> x = 0xFE - 0x100 = -2; ys (bit5) set -> y = 0xFD - 0x100 = -3.
            let events = events(&[0x08 | 0x10 | 0x20, 0xFE, 0xFD]);
            // down = -y = -(-3) = +3 (moved down).
            assert_eq!(events, [MouseEvent::Move { right: -2, down: 3 }]);
        }
    );

    kernel_test!("roxy-ps2::mouse-packet-buttons", emits_only_on_change, {
        // Left press (bit0), no motion.
        let mut result = events(&[0x09, 0x00, 0x00]);
        assert_eq!(result, [MouseEvent::ButtonPressed(MouseButton::Left)]);
        // Same button held: no new event.
        result = events(&[0x09, 0x00, 0x00]);
        assert!(result.is_empty());
        // Release.
        result = events(&[0x08, 0x00, 0x00]);
        assert_eq!(result, [MouseEvent::ButtonReleased(MouseButton::Left)]);
    });

    kernel_test!("roxy-ps2::mouse-packet-multi-button", right_and_middle, {
        // Right (bit1) + Middle (bit2) pressed together, no motion.
        let events = events(&[0x0e, 0x00, 0x00]);
        assert_eq!(
            events,
            [
                MouseEvent::ButtonPressed(MouseButton::Right),
                MouseEvent::ButtonPressed(MouseButton::Middle),
            ]
        );
    });

    kernel_test!(
        "roxy-ps2::mouse-packet-resync",
        discards_non_sync_first_byte,
        {
            // A stray byte without the sync bit is dropped before the real packet.
            let mut parser = MousePacketParser::new();
            assert!(parser.feed(0x55).is_none());
            let batch = parser.feed(0x08).and_then(|_| parser.feed(0x01));
            // 0x08 was only the first byte; nothing complete yet.
            assert!(batch.is_none());
        }
    );

    kernel_test!("roxy-ps2::mouse-packet-wheel", intellimouse_wheel, {
        let mut parser = MousePacketParser::new();
        parser.set_mode(PacketMode::Intellimouse);
        let mut output: Vec<MouseEvent, 5> = Vec::new();
        // wheel byte 0xFF is -1 -> up = +1.
        for &byte in &[0x08, 0x00, 0x00, 0xFF] {
            if let Some(batch) = parser.feed(byte) {
                let _ = output.extend_from_slice(&batch);
            }
        }
        assert_eq!(output, [MouseEvent::Scroll { up: 1 }]);
    });

    kernel_test!(
        "roxy-ps2::mouse-packet-standard-ignores-fourth-byte",
        no_wheel_in_standard,
        {
            let mut parser = MousePacketParser::new();
            parser.set_mode(PacketMode::Standard);
            let mut output: Vec<MouseEvent, 5> = Vec::new();
            for &byte in &[0x08, 0x00, 0x00, 0xFF] {
                if let Some(batch) = parser.feed(byte) {
                    let _ = output.extend_from_slice(&batch);
                }
            }
            assert!(output.is_empty());
        }
    );
}
