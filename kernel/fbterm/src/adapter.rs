use spin::Mutex;

use roxy_boot::FramebufferInfo;
use roxy_terminal::{OutputError, TerminalOutput};
use roxy_tty_types::WindowSize;

use crate::{
    InitError,
    console::Console,
    framebuffer::{Framebuffer, FramebufferLayout},
    renderer::TextRenderer,
};

pub(crate) struct FbTerminal {
    console: Mutex<Console>,
    layout: FramebufferLayout,
}

impl FbTerminal {
    pub(crate) fn new(framebuffers: &[FramebufferInfo]) -> Result<Self, InitError> {
        let framebuffer = framebuffers.first().ok_or(InitError::NoFramebuffer)?;
        let framebuffer = Framebuffer::from_info(framebuffer)?;
        let layout = framebuffer.layout();
        let renderer = TextRenderer::new(framebuffer)?;

        Ok(Self {
            console: Mutex::new(Console::new(renderer)),
            layout,
        })
    }

    pub(crate) fn layout(&self) -> FramebufferLayout {
        self.layout
    }
}

impl TerminalOutput for FbTerminal {
    fn write(&self, input: &[u8]) -> Result<usize, OutputError> {
        self.console.lock().write(input);

        Ok(input.len())
    }

    fn window_size(&self) -> WindowSize {
        self.console.lock().window_size()
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
