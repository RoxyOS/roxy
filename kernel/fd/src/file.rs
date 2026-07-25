use alloc::vec::Vec;

use crate::{IoctlError, IoctlRequest};

pub trait File: Send {
    /// Reports whether this object is a terminal.
    fn is_terminal(&self) -> bool;

    /// Returns metadata for this open object.
    ///
    /// # Errors
    ///
    /// Returns an error when the object does not expose filesystem metadata.
    fn metadata(&self) -> Result<FileMetadata, FileError>;

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

    /// Performs a typed ioctl operation.
    ///
    /// # Errors
    ///
    /// Returns an operation-specific ioctl error.
    fn ioctl(&mut self, _request: IoctlRequest) -> Result<u64, IoctlError> {
        Err(IoctlError::NotTty)
    }

    /// Returns this object's directory capability when it supports directory iteration.
    fn as_directory(&mut self) -> Option<&mut dyn Directory> {
        None
    }
}

pub trait Directory {
    /// Returns up to `limit` directory entries and advances `position` by their count.
    /// `position` is the index of the next directory entry to return.
    ///
    /// # Errors
    ///
    /// Returns an object-specific I/O error.
    fn read_entries(
        &mut self,
        position: &mut u64,
        limit: usize,
    ) -> Result<Vec<DirectoryEntry>, FileError>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectoryEntry {
    pub file_id: u64,
    pub offset: u64,
    pub file_type: FileType,
    pub name: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FileMetadata {
    pub file_id: u64,
    pub file_type: FileType,
    pub permissions: u16,
    pub size: u64,
    pub hard_links: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileType {
    Regular,
    Directory,
    Symlink,
    BlockDevice,
    CharacterDevice,
    Fifo,
    Socket,
    Unknown,
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
