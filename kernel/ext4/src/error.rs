use ext4plus::error::Ext4Error;
use roxy_vfs::VfsError;

#[allow(clippy::needless_pass_by_value)]
pub(crate) fn map_ext4(error: Ext4Error) -> VfsError {
    match error {
        Ext4Error::NotFound => VfsError::NotFound,
        Ext4Error::AlreadyExists => VfsError::AlreadyExists,
        Ext4Error::NotADirectory => VfsError::NotDirectory,
        Ext4Error::IsADirectory => VfsError::IsDirectory,
        Ext4Error::Readonly => VfsError::ReadOnly,
        Ext4Error::NoSpace | Ext4Error::FileTooLarge => VfsError::NoSpace,
        Ext4Error::Io(_) => VfsError::Io,
        Ext4Error::Corrupt(_) => VfsError::Corrupt,
        Ext4Error::NotAbsolute
        | Ext4Error::MalformedPath
        | Ext4Error::PathTooLong
        | Ext4Error::DotEntry => VfsError::InvalidPath,
        Ext4Error::NotASymlink
        | Ext4Error::NotUtf8
        | Ext4Error::TooManySymlinks
        | Ext4Error::Encrypted
        | Ext4Error::IsASpecialFile => VfsError::InvalidInput,
        _ => VfsError::Unsupported,
    }
}
