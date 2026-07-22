use spin::Mutex;

use roxy_arch::{Architecture, CurrentArchitectureBackend};
use roxy_boot::FramebufferInfo;
use roxy_fd::{FileError, FileMetadata, FileType};
use roxy_terminal::TerminalDevice;

use crate::{InitError, console::Console, framebuffer::Framebuffer, renderer::TextRenderer};

pub(crate) struct FbTerminal {
    console: Mutex<Console>,
}

impl FbTerminal {
    pub(crate) fn new(framebuffers: &[FramebufferInfo]) -> Result<Self, InitError> {
        let framebuffer = framebuffers.first().ok_or(InitError::NoFramebuffer)?;
        let framebuffer = Framebuffer::from_info(framebuffer)?;
        let renderer = TextRenderer::new(framebuffer)?;

        Ok(Self {
            console: Mutex::new(Console::new(renderer)),
        })
    }
}

impl TerminalDevice for FbTerminal {
    fn metadata(&self) -> FileMetadata {
        FileMetadata {
            file_id: 2,
            file_type: FileType::CharacterDevice,
            permissions: 0o600,
            size: 0,
            hard_links: 1,
        }
    }

    fn read(&self, output: &mut [u8]) -> Result<usize, FileError> {
        if output.is_empty() {
            return Ok(0);
        }

        loop {
            let mut count = 0;

            while count < output.len() {
                let Some(byte) = roxy_ps2::read() else {
                    break;
                };

                output[count] = byte;
                count += 1;
            }

            if count > 0 {
                return Ok(count);
            }

            assert!(CurrentArchitectureBackend::interrupts_enabled());
            CurrentArchitectureBackend::halt();
        }
    }

    fn write(&self, input: &[u8]) -> Result<usize, FileError> {
        self.console.lock().write(input);

        Ok(input.len())
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use alloc::vec;

    use roxy_boot::FramebufferInfo;
    use roxy_terminal::TerminalDevice;
    use roxy_test::kernel_test;

    use super::FbTerminal;
    use crate::InitError;

    kernel_test!("roxy-fbterm::missing", rejects_missing_framebuffer, {
        assert!(matches!(
            FbTerminal::new(&[]),
            Err(InitError::NoFramebuffer)
        ));
    });

    kernel_test!("roxy-fbterm::keyboard-input", delegates_keyboard_input, {
        let mut storage = vec![0u8; 16 * 32 * 4];
        let framebuffer = FramebufferInfo {
            address: storage.as_mut_ptr() as u64,
            width: 16,
            height: 32,
            pitch: 64,
            bits_per_pixel: 32,
            memory_model: 1,
            red_mask_size: 8,
            red_mask_shift: 16,
            green_mask_size: 8,
            green_mask_shift: 8,
            blue_mask_size: 8,
            blue_mask_shift: 0,
        };
        let terminal = FbTerminal::new(&[framebuffer]).unwrap();
        roxy_ps2::inject_for_test(b"ok\n");

        let mut output = [0; 3];
        let count = terminal.read(&mut output).unwrap();
        assert_eq!(count, 3);
        assert_eq!(&output, b"ok\n");
    });
}
