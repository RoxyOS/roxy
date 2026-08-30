use core::hint::spin_loop;

use x86_64::instructions::port::{Port, PortWriteOnly};

const DATA_PORT: u16 = 0x60;
const COMMAND_PORT: u16 = 0x64;
const STATUS_OUTPUT_FULL: u8 = 1 << 0;
const STATUS_INPUT_FULL: u8 = 1 << 1;
const CONFIG_IRQ1: u8 = 1 << 0;
const CONFIG_IRQ2: u8 = 1 << 1;
const CONFIG_TRANSLATION: u8 = 1 << 6;
const COMMAND_DISABLE_FIRST: u8 = 0xad;
const COMMAND_READ_CONFIG: u8 = 0x20;
const COMMAND_WRITE_CONFIG: u8 = 0x60;
const COMMAND_ENABLE_FIRST: u8 = 0xae;
const KEYBOARD_RESET: u8 = 0xff;
const KEYBOARD_ENABLE_SCANNING: u8 = 0xf4;
const KEYBOARD_ACK: u8 = 0xfa;
const KEYBOARD_SELF_TEST_OK: u8 = 0xaa;
const COMMAND_ENABLE_SECOND: u8 = 0xa8;
const COMMAND_WRITE_PORT2: u8 = 0xd4;
const MOUSE_RESET: u8 = 0xff;
const MOUSE_ENABLE_REPORTING: u8 = 0xf4;
const MOUSE_ACK: u8 = 0xfa;
const MOUSE_BAT_OK: u8 = 0xaa;
const MOUSE_STANDARD_ID: u8 = 0x00;
const TIMEOUT: usize = 100_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum InitError {
    Timeout,
    ResetFailed,
    EnableFailed,
}

/// Owns i8042 command/status I/O and its first keyboard data port.
pub(crate) struct I8042FirstPort {
    data: Port<u8>,
    status: Port<u8>,
    command: PortWriteOnly<u8>,
}

impl I8042FirstPort {
    pub(crate) fn initialize() -> Result<(), InitError> {
        let mut first_port = Self::new();

        first_port.wait_input_clear()?;
        first_port.write_command(COMMAND_DISABLE_FIRST)?;
        first_port.drain_output();
        first_port.write_command(COMMAND_READ_CONFIG)?;

        let mut config = first_port.read_response()?;
        config = (config | CONFIG_IRQ1 | CONFIG_TRANSLATION) & !CONFIG_IRQ2;

        first_port.wait_input_clear()?;
        first_port.write_command(COMMAND_WRITE_CONFIG)?;
        first_port.write_data(config)?;
        first_port.wait_input_clear()?;
        first_port.write_command(COMMAND_ENABLE_FIRST)?;
        first_port.reset_keyboard()?;
        first_port.enable_scanning()
    }

    pub(crate) fn read_data() -> u8 {
        let mut data = Port::<u8>::new(DATA_PORT);
        // SAFETY: IRQ1 is delivered only after the i8042 output buffer is full, and this port is
        // exclusively owned by the PS/2 driver.
        unsafe { data.read() }
    }

    fn new() -> Self {
        Self {
            data: Port::new(DATA_PORT),
            status: Port::new(COMMAND_PORT),
            command: PortWriteOnly::new(COMMAND_PORT),
        }
    }

    fn read_status(&mut self) -> u8 {
        // SAFETY: The status port is fixed by the i8042 specification and exclusively owned here.
        unsafe { self.status.read() }
    }

    fn read_response(&mut self) -> Result<u8, InitError> {
        for _ in 0..TIMEOUT {
            if self.read_status() & STATUS_OUTPUT_FULL != 0 {
                // SAFETY: The output-full status bit guarantees a byte is available at 0x60.
                return Ok(unsafe { self.data.read() });
            }

            spin_loop();
        }

        Err(InitError::Timeout)
    }

    fn wait_input_clear(&mut self) -> Result<(), InitError> {
        for _ in 0..TIMEOUT {
            if self.read_status() & STATUS_INPUT_FULL == 0 {
                return Ok(());
            }

            spin_loop();
        }

        Err(InitError::Timeout)
    }

    fn drain_output(&mut self) {
        for _ in 0..TIMEOUT {
            if self.read_status() & STATUS_OUTPUT_FULL == 0 {
                return;
            }
            // SAFETY: The output-full status bit guarantees a byte is available at 0x60.
            let _ = unsafe { self.data.read() };
        }
    }

    fn write_command(&mut self, command: u8) -> Result<(), InitError> {
        self.wait_input_clear()?;
        // SAFETY: The command port is fixed by the i8042 specification and exclusively owned here.
        unsafe { self.command.write(command) };

        Ok(())
    }

    fn write_data(&mut self, data: u8) -> Result<(), InitError> {
        self.wait_input_clear()?;
        // SAFETY: The data port is fixed by the i8042 specification and exclusively owned here.
        unsafe { self.data.write(data) };

        Ok(())
    }

    fn reset_keyboard(&mut self) -> Result<(), InitError> {
        self.write_data(KEYBOARD_RESET)?;
        let mut acknowledged = false;

        for _ in 0..TIMEOUT {
            if self.read_status() & STATUS_OUTPUT_FULL == 0 {
                spin_loop();
                continue;
            }

            let response = self.read_response()?;

            if response == KEYBOARD_ACK {
                acknowledged = true;
            } else if acknowledged && response == KEYBOARD_SELF_TEST_OK {
                return Ok(());
            }
        }

        Err(InitError::ResetFailed)
    }

    fn enable_scanning(&mut self) -> Result<(), InitError> {
        self.write_data(KEYBOARD_ENABLE_SCANNING)?;

        for _ in 0..TIMEOUT {
            if self.read_status() & STATUS_OUTPUT_FULL != 0 {
                return (self.read_response()? == KEYBOARD_ACK)
                    .then_some(())
                    .ok_or(InitError::EnableFailed);
            }

            spin_loop();
        }

        Err(InitError::EnableFailed)
    }
}

/// Owns i8042 second-port (auxiliary/mouse) configuration and data reads.
///
/// The second port shares the controller's data/status/command registers with the first port.
/// Sending a byte to the mouse requires routing it through the `write-to-port-2` command (0xd4).
pub(crate) struct I8042SecondPort {
    data: Port<u8>,
    status: Port<u8>,
    command: PortWriteOnly<u8>,
}

impl I8042SecondPort {
    /// Enables the auxiliary device, enables IRQ2, and starts mouse data reporting.
    ///
    /// Unlike the keyboard, a missing or failed mouse is tolerated: the controller may simply
    /// have no second port (common on real hardware without an aux device), so failure is
    /// reported to the caller rather than panicking.
    pub(crate) fn initialize() -> Result<(), InitError> {
        let mut port = Self::new();

        port.wait_input_clear()?;
        port.write_command(COMMAND_ENABLE_SECOND)?;
        port.drain_output();
        port.write_command(COMMAND_READ_CONFIG)?;

        let mut config = port.read_response()?;
        config |= CONFIG_IRQ2;

        port.wait_input_clear()?;
        port.write_command(COMMAND_WRITE_CONFIG)?;
        port.write_data(config)?;
        port.wait_input_clear()?;
        port.reset_mouse()?;
        port.enable_reporting()
    }

    pub(crate) fn read_data() -> u8 {
        let mut data = Port::<u8>::new(DATA_PORT);
        // SAFETY: IRQ12 is delivered only after the i8042 output buffer is full, and this port is
        // exclusively owned by the PS/2 driver.
        unsafe { data.read() }
    }

    fn new() -> Self {
        Self {
            data: Port::new(DATA_PORT),
            status: Port::new(COMMAND_PORT),
            command: PortWriteOnly::new(COMMAND_PORT),
        }
    }

    fn read_status(&mut self) -> u8 {
        // SAFETY: The status port is fixed by the i8042 specification and exclusively owned here.
        unsafe { self.status.read() }
    }

    fn read_response(&mut self) -> Result<u8, InitError> {
        for _ in 0..TIMEOUT {
            if self.read_status() & STATUS_OUTPUT_FULL != 0 {
                // SAFETY: The output-full status bit guarantees a byte is available at 0x60.
                return Ok(unsafe { self.data.read() });
            }

            spin_loop();
        }

        Err(InitError::Timeout)
    }

    fn wait_input_clear(&mut self) -> Result<(), InitError> {
        for _ in 0..TIMEOUT {
            if self.read_status() & STATUS_INPUT_FULL == 0 {
                return Ok(());
            }

            spin_loop();
        }

        Err(InitError::Timeout)
    }

    fn drain_output(&mut self) {
        for _ in 0..TIMEOUT {
            if self.read_status() & STATUS_OUTPUT_FULL == 0 {
                return;
            }
            // SAFETY: The output-full status bit guarantees a byte is available at 0x60.
            let _ = unsafe { self.data.read() };
        }
    }

    fn write_command(&mut self, command: u8) -> Result<(), InitError> {
        self.wait_input_clear()?;
        // SAFETY: The command port is fixed by the i8042 specification and exclusively owned here.
        unsafe { self.command.write(command) };

        Ok(())
    }

    /// Sends one command byte to the mouse via the write-to-port-2 (0xd4) routing command.
    fn write_mouse_command(&mut self, command: u8) -> Result<(), InitError> {
        self.write_command(COMMAND_WRITE_PORT2)?;
        self.write_data(command)
    }

    pub(crate) fn write_mouse_byte(byte: u8) -> Result<(), InitError> {
        let mut port = Self::new();
        port.write_mouse_command(byte)
    }

    fn write_data(&mut self, data: u8) -> Result<(), InitError> {
        self.wait_input_clear()?;
        // SAFETY: The data port is fixed by the i8042 specification and exclusively owned here.
        unsafe { self.data.write(data) };

        Ok(())
    }

    fn reset_mouse(&mut self) -> Result<(), InitError> {
        self.write_mouse_command(MOUSE_RESET)?;
        let mut acknowledged = false;

        for _ in 0..TIMEOUT {
            if self.read_status() & STATUS_OUTPUT_FULL == 0 {
                spin_loop();
                continue;
            }

            let response = self.read_response()?;

            if response == MOUSE_ACK {
                acknowledged = true;
            } else if acknowledged && response == MOUSE_BAT_OK {
                // A standard PS/2 mouse sends its device ID after BAT. Consume it as
                // part of the reset handshake so it cannot be mistaken for a later ACK.
                let device_id = self.read_response()?;
                if device_id != MOUSE_STANDARD_ID {
                    return Err(InitError::ResetFailed);
                }
                return Ok(());
            }
        }

        Err(InitError::ResetFailed)
    }

    fn enable_reporting(&mut self) -> Result<(), InitError> {
        self.write_mouse_command(MOUSE_ENABLE_REPORTING)?;

        for _ in 0..TIMEOUT {
            if self.read_status() & STATUS_OUTPUT_FULL != 0 {
                return (self.read_response()? == MOUSE_ACK)
                    .then_some(())
                    .ok_or(InitError::EnableFailed);
            }

            spin_loop();
        }

        Err(InitError::EnableFailed)
    }
}
