use roxy_tty_types::WindowSize;

/// The display/consumption endpoint a terminal writes its output and echo to.
///
/// A console terminal implements this over the kernel framebuffer or serial output; a pty slave
/// implements it over the pty master's receive side.
pub trait TtyOutput: Send + Sync {
    /// Writes bytes to the terminal output.
    ///
    /// # Errors
    ///
    /// Returns an output error when the endpoint cannot accept bytes.
    fn write(&self, input: &[u8]) -> Result<usize, OutputError>;

    /// Returns this endpoint's display size, or `WindowSize::UNKNOWN` when it has none.
    fn window_size(&self) -> WindowSize;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputError {
    Io,
}
