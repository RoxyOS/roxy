pub trait File: Send {
    /// Reads data from this open object.
    ///
    /// # Errors
    ///
    /// Returns an object-specific I/O or unsupported-operation error.
    fn read(&mut self, output: &mut [u8]) -> Result<usize, FileError>;

    /// Writes data to this open object.
    ///
    /// # Errors
    ///
    /// Returns an object-specific I/O or unsupported-operation error.
    fn write(&mut self, input: &[u8]) -> Result<usize, FileError>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileError {
    BadOperation,
    Io,
}
