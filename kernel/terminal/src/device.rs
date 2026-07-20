use roxy_fd::{FileError, FileMetadata};

/// A shared terminal endpoint that can back one or more open files.
pub trait TerminalDevice: Send + Sync {
    /// Returns metadata describing this terminal endpoint.
    fn metadata(&self) -> FileMetadata;

    /// Reads bytes according to the endpoint's blocking policy.
    ///
    /// # Errors
    ///
    /// Returns an endpoint-specific I/O error.
    fn read(&self, output: &mut [u8]) -> Result<usize, FileError>;

    /// Writes bytes to the endpoint.
    ///
    /// # Errors
    ///
    /// Returns an endpoint-specific I/O error.
    fn write(&self, input: &[u8]) -> Result<usize, FileError>;
}
