use crate::OpenFile;

pub use roxy_fb_types::{FbBitfield, FbFixedInfo, FbVarInfo};
pub use roxy_tty_types::{ApplyWhen, LocalFlags, Termios, WindowSize};

#[derive(Debug)]
pub enum IoctlRequest<'a> {
    GetTermios(&'a mut Termios),
    SetTermios { when: ApplyWhen, termios: Termios },
    GetWindowSize(&'a mut WindowSize),
    SetWindowSize(WindowSize),
    GetForegroundPgid(&'a mut u32),
    SetForegroundPgid(u32),
    FbGetVarInfo(&'a mut FbVarInfo),
    FbSetVarInfo(FbVarInfo),
    FbGetFixedInfo(&'a mut FbFixedInfo),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IoctlError {
    NotTty,
    /// The request is supported but its arguments are not (reported as `EINVAL`).
    Invalid,
    Unsupported {
        operation: &'static str,
        argument: u64,
    },
}

/// Describes the physical memory backing a file-backed `mmap`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MmapTarget {
    pub physical_address: u64,
    pub length: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MmapError {
    Unsupported,
    InvalidArgument,
}

impl OpenFile {
    /// Dispatches an ioctl request while holding the serialized open-file state.
    ///
    /// # Errors
    ///
    /// Returns the underlying object's ioctl error.
    pub fn ioctl(&self, request: IoctlRequest<'_>) -> Result<(), IoctlError> {
        let mut state = self.state.lock();

        state.object.ioctl(request)
    }

    /// Maps a file object's physical memory for a file-backed `mmap`.
    ///
    /// # Errors
    ///
    /// Returns the underlying object's mapping error.
    pub fn mmap(&self, size: usize, offset: u64) -> Result<MmapTarget, MmapError> {
        let mut state = self.state.lock();

        state.object.mmap(size, offset)
    }
}
