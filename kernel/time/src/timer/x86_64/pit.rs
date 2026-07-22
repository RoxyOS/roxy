use core::hint::spin_loop;

use x86_64::instructions::port::{Port, PortWriteOnly};

pub(super) const FREQUENCY_HZ: u64 = 1_193_182;
pub(super) const CALIBRATION_RELOAD: u16 = 59_659;

const CHANNEL_TWO_PORT: u16 = 0x42;
const COMMAND_PORT: u16 = 0x43;
const SPEAKER_PORT: u16 = 0x61;
const CHANNEL_TWO_MODE_ZERO: u8 = 0xb0;
const GATE_TWO: u8 = 1 << 0;
const SPEAKER_DATA: u8 = 1 << 1;
const OUTPUT_TWO: u8 = 1 << 5;

pub(super) fn wait_calibration_window() {
    let mut channel = PortWriteOnly::<u8>::new(CHANNEL_TWO_PORT);
    let mut command = PortWriteOnly::<u8>::new(COMMAND_PORT);
    let mut speaker = Port::<u8>::new(SPEAKER_PORT);

    // SAFETY: These ports exclusively configure and poll PIT channel 2 while interrupts are disabled.
    unsafe {
        let original = speaker.read();
        let [low, high] = CALIBRATION_RELOAD.to_le_bytes();
        speaker.write(original & !(GATE_TWO | SPEAKER_DATA));
        command.write(CHANNEL_TWO_MODE_ZERO);
        channel.write(low);
        channel.write(high);
        speaker.write((original & !SPEAKER_DATA) | GATE_TWO);

        while speaker.read() & OUTPUT_TWO == 0 {
            spin_loop();
        }
        speaker.write(original);
    }
}
