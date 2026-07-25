use roxy_tty_types::WindowSize;

/// A shared terminal display endpoint.
pub trait TerminalOutput: Send + Sync {
    /// Writes bytes to the terminal display.
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
