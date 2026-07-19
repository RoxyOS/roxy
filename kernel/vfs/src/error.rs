use core::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VfsError {
    NotInitialized,
    InvalidPath,
    InvalidInput,
    NotFound,
    AlreadyExists,
    NotDirectory,
    IsDirectory,
    DirectoryNotEmpty,
    ReadOnly,
    PermissionDenied,
    NoSpace,
    Busy,
    CrossDevice,
    Unsupported,
    Io,
    Corrupt,
}

impl fmt::Display for VfsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::NotInitialized => "root filesystem is not initialized",
            Self::InvalidPath => "invalid path",
            Self::InvalidInput => "invalid input",
            Self::NotFound => "entry not found",
            Self::AlreadyExists => "entry already exists",
            Self::NotDirectory => "entry is not a directory",
            Self::IsDirectory => "entry is a directory",
            Self::DirectoryNotEmpty => "directory is not empty",
            Self::ReadOnly => "filesystem is read-only",
            Self::PermissionDenied => "permission denied",
            Self::NoSpace => "no space left on device",
            Self::Busy => "resource is busy",
            Self::CrossDevice => "operation crosses mount points",
            Self::Unsupported => "operation is unsupported",
            Self::Io => "filesystem I/O failed",
            Self::Corrupt => "filesystem is corrupt",
        })
    }
}

impl core::error::Error for VfsError {}
