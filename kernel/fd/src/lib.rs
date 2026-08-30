#![no_std]

extern crate alloc;

mod fd;
mod file;
mod ioctl;
mod open;
mod socket;
mod status_flags;
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
pub use socket::{ShutdownHow, SocketError, SocketOps, SockoptLevel, SockoptName};
pub use status_flags::StatusFlags;
pub use table::DupError;
pub use table::FdTable;
