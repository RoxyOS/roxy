use alloc::sync::Arc;

use roxy_fd::{
    File as FdFile, FileError as FdFileError, FileMetadata as FdFileMetadata,
    FileType as FdFileType, IoctlError, IoctlRequest, MmapError, MmapTarget, PollEvents,
    SeekError as FdSeekError, SeekFrom as FdSeekFrom, TruncateError as FdTruncateError,
};
use roxy_poll::{PollListener, PollRegistration};

use crate::{FilePermissions, SeekFrom as VfsSeekFrom, VfsError, VfsFile};

// Calls to `self.{seek, read, write}` are calling `VfsFile::{seek, read, write}`,
// not to be confused with `FdFile::{seek, read, write}`
impl FdFile for VfsFile {
    fn poll(&mut self) -> Result<PollEvents, FdFileError> {
        VfsFile::poll(self).map_err(map_file_error)
    }

    fn register_poll_listener(&mut self, listener: Arc<PollListener>) -> PollRegistration {
        VfsFile::register_poll_listener(self, listener)
    }

    fn is_terminal(&self) -> bool {
        false
    }

    fn metadata(&self) -> Result<FdFileMetadata, FdFileError> {
        let metadata = self.metadata().map_err(map_file_error)?;

        Ok(FdFileMetadata {
            file_id: metadata.file_id,
            file_type: map_file_type(metadata.file_type),
            permissions: metadata.permissions.bits(),
            size: metadata.size,
            hard_links: metadata.hard_links,
        })
    }

    fn read(
        &mut self,
        position: &mut u64,
        output: &mut [u8],
        _nonblocking: bool,
    ) -> Result<usize, FdFileError> {
        self.seek(VfsSeekFrom::Start(*position))
            .map_err(map_file_error)?;

        let read = self.read(output).map_err(map_file_error)?;

        *position = self.seek(VfsSeekFrom::Current(0)).map_err(map_file_error)?;

        Ok(read)
    }

    fn write(
        &mut self,
        position: &mut u64,
        input: &[u8],
        _nonblocking: bool,
    ) -> Result<usize, FdFileError> {
        self.seek(VfsSeekFrom::Start(*position))
            .map_err(map_file_error)?;

        let written = self.write(input).map_err(map_file_error)?;

        *position = self.seek(VfsSeekFrom::Current(0)).map_err(map_file_error)?;

        Ok(written)
    }

    fn sync(&mut self) -> Result<(), FdFileError> {
        self.sync().map_err(map_file_error)
    }

    fn truncate(&mut self, size: u64) -> Result<(), FdTruncateError> {
        self.truncate(size).map_err(map_truncate_error)
    }

    fn set_permissions(&mut self, permissions: u16) -> Result<(), FdFileError> {
        let permissions = FilePermissions::new(permissions).ok_or(FdFileError::BadOperation)?;
        self.set_permissions(permissions).map_err(map_file_error)
    }

    fn seek(&mut self, current: u64, position: FdSeekFrom) -> Result<u64, FdSeekError> {
        self.seek(VfsSeekFrom::Start(current))
            .map_err(map_seek_error)?;

        self.seek(match position {
            FdSeekFrom::Start(position) => VfsSeekFrom::Start(position),
            FdSeekFrom::Current(offset) => VfsSeekFrom::Current(offset),
            FdSeekFrom::End(offset) => VfsSeekFrom::End(offset),
        })
        .map_err(map_seek_error)
    }

    fn ioctl(&mut self, request: IoctlRequest<'_>) -> Result<(), IoctlError> {
        self.ioctl(request)
    }

    fn mmap(&mut self, size: usize, offset: u64) -> Result<MmapTarget, MmapError> {
        self.mmap(size, offset)
    }
}

fn map_file_error(error: VfsError) -> FdFileError {
    match error {
        VfsError::InvalidInput
        | VfsError::IsDirectory
        | VfsError::NotDirectory
        | VfsError::PermissionDenied
        | VfsError::Unsupported => FdFileError::BadOperation,
        VfsError::WouldBlock => FdFileError::WouldBlock,
        _ => FdFileError::Io,
    }
}

fn map_seek_error(error: VfsError) -> FdSeekError {
    match error {
        VfsError::InvalidInput => FdSeekError::InvalidOffset,
        VfsError::IsDirectory | VfsError::NotDirectory | VfsError::Unsupported => {
            FdSeekError::NotSeekable
        }
        _ => FdSeekError::Io,
    }
}

fn map_truncate_error(error: VfsError) -> FdTruncateError {
    match error {
        VfsError::PermissionDenied => FdTruncateError::PermissionDenied,
        VfsError::ReadOnly => FdTruncateError::ReadOnly,
        VfsError::InvalidInput => FdTruncateError::InvalidSize,
        VfsError::NoSpace => FdTruncateError::NoSpace,
        VfsError::Unsupported | VfsError::IsDirectory | VfsError::NotDirectory => {
            FdTruncateError::BadOperation
        }
        _ => FdTruncateError::Io,
    }
}

fn map_file_type(file_type: crate::FileType) -> FdFileType {
    match file_type {
        crate::FileType::Regular => FdFileType::Regular,
        crate::FileType::Directory => FdFileType::Directory,
        crate::FileType::Symlink => FdFileType::Symlink,
        crate::FileType::BlockDevice => FdFileType::BlockDevice,
        crate::FileType::CharacterDevice => FdFileType::CharacterDevice,
        crate::FileType::Fifo => FdFileType::Fifo,
        crate::FileType::Socket => FdFileType::Socket,
        crate::FileType::Unknown => FdFileType::Unknown,
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use alloc::{boxed::Box, sync::Arc, vec::Vec};

    use crate::{
        DirEntry, FileHandle, FilePermissions, FileSystem, FileType, Metadata, OpenAccess,
        OpenOptions, ResolvedPath, SeekFrom as VfsSeekFrom, Vfs, VfsError,
    };

    use roxy_fd::{OpenFile as FdOpenFile, SeekFrom as FdSeekFrom};

    struct MockFileSystem;

    struct Cursor {
        data: Vec<u8>,
        position: usize,
    }

    impl FileSystem for MockFileSystem {
        fn open(
            &self,
            _path: &ResolvedPath,
            _options: OpenOptions,
        ) -> Result<Box<dyn FileHandle>, VfsError> {
            Ok(Box::new(Cursor {
                data: b"hello".to_vec(),
                position: 0,
            }))
        }

        fn metadata(&self, _path: &ResolvedPath, _follow: bool) -> Result<Metadata, VfsError> {
            Ok(metadata())
        }

        fn read_dir(&self, _path: &ResolvedPath) -> Result<Vec<DirEntry>, VfsError> {
            Err(VfsError::Unsupported)
        }

        fn mkdir(
            &self,
            _path: &ResolvedPath,
            _permissions: FilePermissions,
        ) -> Result<(), VfsError> {
            Err(VfsError::Unsupported)
        }

        fn rmdir(&self, _path: &ResolvedPath) -> Result<(), VfsError> {
            Err(VfsError::Unsupported)
        }

        fn unlink(&self, _path: &ResolvedPath) -> Result<(), VfsError> {
            Err(VfsError::Unsupported)
        }

        fn hard_link(
            &self,
            _source: &ResolvedPath,
            _destination: &ResolvedPath,
        ) -> Result<(), VfsError> {
            Err(VfsError::Unsupported)
        }

        fn symlink(&self, _target: &[u8], _link: &ResolvedPath) -> Result<(), VfsError> {
            Err(VfsError::Unsupported)
        }

        fn read_link(&self, _path: &ResolvedPath) -> Result<Vec<u8>, VfsError> {
            Err(VfsError::Unsupported)
        }

        fn rename(
            &self,
            _source: &ResolvedPath,
            _destination: &ResolvedPath,
        ) -> Result<(), VfsError> {
            Err(VfsError::Unsupported)
        }

        fn set_permissions(
            &self,
            _path: &ResolvedPath,
            _permissions: FilePermissions,
        ) -> Result<(), VfsError> {
            Err(VfsError::Unsupported)
        }

        fn sync(&self) -> Result<(), VfsError> {
            Ok(())
        }
    }

    impl FileHandle for Cursor {
        fn read(&mut self, output: &mut [u8]) -> Result<usize, VfsError> {
            let length = output
                .len()
                .min(self.data.len().saturating_sub(self.position));

            output[..length].copy_from_slice(&self.data[self.position..self.position + length]);
            self.position += length;

            Ok(length)
        }

        fn write(&mut self, input: &[u8]) -> Result<usize, VfsError> {
            let end = self
                .position
                .checked_add(input.len())
                .ok_or(VfsError::InvalidInput)?;

            if end > self.data.len() {
                self.data.resize(end, 0);
            }

            self.data[self.position..end].copy_from_slice(input);
            self.position = end;

            Ok(input.len())
        }

        fn seek(&mut self, position: VfsSeekFrom) -> Result<u64, VfsError> {
            let position = match position {
                VfsSeekFrom::Start(position) => position,
                VfsSeekFrom::Current(offset) => add_offset(self.position, offset)?,
                VfsSeekFrom::End(offset) => add_offset(self.data.len(), offset)?,
            };
            self.position = usize::try_from(position).map_err(|_| VfsError::InvalidInput)?;

            Ok(position)
        }

        fn truncate(&mut self, size: u64) -> Result<(), VfsError> {
            self.data.resize(
                usize::try_from(size).map_err(|_| VfsError::InvalidInput)?,
                0,
            );

            Ok(())
        }

        fn metadata(&self) -> Result<Metadata, VfsError> {
            Ok(Metadata {
                size: u64::try_from(self.data.len()).map_err(|_| VfsError::InvalidInput)?,
                ..metadata()
            })
        }

        fn sync(&mut self) -> Result<(), VfsError> {
            Ok(())
        }

        fn set_permissions(&mut self, _permissions: FilePermissions) -> Result<(), VfsError> {
            Err(VfsError::Unsupported)
        }
    }

    fn metadata() -> Metadata {
        Metadata {
            file_id: 1,
            file_type: FileType::Regular,
            permissions: crate::FilePermissions::DEFAULT_FILE,
            size: 5,
            hard_links: 1,
        }
    }

    fn add_offset(base: usize, offset: i64) -> Result<u64, VfsError> {
        let base = u64::try_from(base).map_err(|_| VfsError::InvalidInput)?;

        base.checked_add_signed(offset)
            .ok_or(VfsError::InvalidInput)
    }

    roxy_test::kernel_test!("roxy-vfs::fd-file-adapter", synchronizes_vfs_position, {
        let vfs = Vfs::new();

        vfs.mount(ResolvedPath::root(), Arc::new(MockFileSystem))
            .unwrap();

        let file = vfs
            .open(
                &ResolvedPath::resolve(b"/file").unwrap(),
                OpenOptions {
                    access: OpenAccess::ReadWrite,
                    ..OpenOptions::read_only()
                },
            )
            .unwrap();
        let file = FdOpenFile::new(Box::new(file));
        let mut output = [0; 5];

        assert_eq!(file.read(&mut output[..2]), Ok(2));
        assert_eq!(file.seek(FdSeekFrom::Current(1)), Ok(3));
        assert_eq!(file.write(b"X"), Ok(1));
        assert_eq!(file.seek(FdSeekFrom::Start(0)), Ok(0));
        assert_eq!(file.read(&mut output), Ok(5));
        assert_eq!(&output, b"helXo");
        assert_eq!(file.metadata().unwrap().size, 5);
        assert_eq!(file.metadata().unwrap().permissions, 0o644);
        assert_eq!(file.truncate(2), Ok(()));
        assert_eq!(file.metadata().unwrap().size, 2);
        assert_eq!(file.seek(FdSeekFrom::Current(0)), Ok(5));
        assert_eq!(file.truncate(7), Ok(()));
        assert_eq!(file.seek(FdSeekFrom::Start(2)), Ok(2));
        assert_eq!(file.read(&mut output), Ok(5));
        assert_eq!(&output, &[0; 5]);
    });
}
