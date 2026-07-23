/// A shared terminal display endpoint.
pub trait TerminalOutput: Send + Sync {
    /// Writes bytes to the terminal display.
    ///
    /// # Errors
    ///
    /// Returns an output error when the endpoint cannot accept bytes.
    fn write(&self, input: &[u8]) -> Result<usize, OutputError>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputError {
    Io,
}
