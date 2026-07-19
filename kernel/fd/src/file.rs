pub trait File: Send {
    /// Reads data from this open object.
    ///
    /// `position` is owned by `OpenFile`; seekable implementations advance it by the consumed
    /// byte count, while stream-like implementations may leave it unchanged.
    ///
    /// # Errors
    ///
    /// Returns an object-specific I/O or unsupported-operation error.
    fn read(&mut self, position: &mut u64, output: &mut [u8]) -> Result<usize, FileError>;

    /// Writes data to this open object.
    ///
    /// `position` is owned by `OpenFile`; seekable implementations advance it by the produced
    /// byte count, while stream-like implementations may leave it unchanged.
    ///
    /// # Errors
    ///
    /// Returns an object-specific I/O or unsupported-operation error.
    fn write(&mut self, position: &mut u64, input: &[u8]) -> Result<usize, FileError>;

    /// Returns the absolute byte offset to seek to.
    ///
    /// # Errors
    ///
    /// Returns an error when the object is not seekable or the target position is invalid.
    fn seek(&mut self, current: u64, position: SeekFrom) -> Result<u64, SeekError>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileError {
    BadOperation,
    Io,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeekFrom {
    Start(u64),
    Current(i64),
    End(i64),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeekError {
    NotSeekable,
    InvalidOffset,
    Overflow,
    Io,
}
