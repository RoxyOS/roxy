#![no_std]

extern crate alloc;

mod fd;
mod file;
mod ioctl;
mod open;
mod table;
mod truncate;

pub use fd::Fd;
pub use file::{
    Directory, DirectoryEntry, File, FileError, FileMetadata, FileType, PollEvents, SeekError,
    SeekFrom, TruncateError,
};
pub use ioctl::{ApplyWhen, IoctlError, IoctlRequest, LocalFlags, Termios, WindowSize};
pub use open::OpenFile;
pub use table::FdTable;
