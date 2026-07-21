use roxy_boot::FramebufferInfo;
use roxy_fd::{FileError, FileMetadata, FileType};
use roxy_process::current_process_id;
use roxy_terminal::TerminalDevice;
use roxy_thread::scheduler::current_thread_id;
use spin::Mutex;

use crate::{InitError, console::Console, framebuffer::Framebuffer, renderer::TextRenderer};

const BAD_FD_ERRNO: u64 = 9;

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
        roxy_utils::unsupported::report(
            "fbterm.read",
            output.len(),
            current_process_id(),
            current_thread_id(),
            BAD_FD_ERRNO,
        );

        Err(FileError::BadOperation)
    }

    fn write(&self, input: &[u8]) -> Result<usize, FileError> {
        self.console.lock().write(input);

        Ok(input.len())
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_test::kernel_test;

    use super::FbTerminal;
    use crate::InitError;

    kernel_test!("roxy-fbterm::missing", rejects_missing_framebuffer, {
        assert!(matches!(
            FbTerminal::new(&[]),
            Err(InitError::NoFramebuffer)
        ));
    });
}
