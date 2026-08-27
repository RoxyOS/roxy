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
pub use ioctl::{
    ApplyWhen, FbBitfield, FbFixedInfo, FbVarInfo, IoctlError, IoctlRequest, LocalFlags, MmapError,
    MmapTarget, Termios, WindowSize,
};
pub use open::OpenFile;
pub use table::FdTable;
