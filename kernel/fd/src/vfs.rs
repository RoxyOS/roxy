use alloc::{boxed::Box, sync::Arc};

use roxy_vfs::{SeekFrom as VfsSeekFrom, VfsError, VfsFile};

use crate::{File, FileError, FileMetadata, FileType, OpenFile, SeekError, SeekFrom};

struct VfsFileObject {
    file: VfsFile,
}

impl OpenFile {
    #[must_use]
    pub fn from_vfs(file: VfsFile) -> Arc<Self> {
        Self::new(Box::new(VfsFileObject { file }))
    }
}

impl File for VfsFileObject {
    fn is_terminal(&self) -> bool {
        false
    }

    fn metadata(&self) -> Result<FileMetadata, FileError> {
        let metadata = self.file.metadata().map_err(map_file_error)?;

        Ok(FileMetadata {
            file_id: metadata.file_id,
            file_type: map_file_type(metadata.file_type),
            permissions: metadata.permissions.bits(),
            size: metadata.size,
            hard_links: metadata.hard_links,
        })
    }

    fn read(&mut self, position: &mut u64, output: &mut [u8]) -> Result<usize, FileError> {
        self.file
            .seek(VfsSeekFrom::Start(*position))
            .map_err(map_file_error)?;

        let read = self.file.read(output).map_err(map_file_error)?;

        *position = self
            .file
            .seek(VfsSeekFrom::Current(0))
            .map_err(map_file_error)?;

        Ok(read)
    }

    fn write(&mut self, position: &mut u64, input: &[u8]) -> Result<usize, FileError> {
        self.file
            .seek(VfsSeekFrom::Start(*position))
            .map_err(map_file_error)?;

        let written = self.file.write(input).map_err(map_file_error)?;

        *position = self
            .file
            .seek(VfsSeekFrom::Current(0))
            .map_err(map_file_error)?;

        Ok(written)
    }

    fn seek(&mut self, current: u64, position: SeekFrom) -> Result<u64, SeekError> {
        self.file
            .seek(VfsSeekFrom::Start(current))
            .map_err(map_seek_error)?;

        self.file
            .seek(match position {
                SeekFrom::Start(position) => VfsSeekFrom::Start(position),
                SeekFrom::Current(offset) => VfsSeekFrom::Current(offset),
                SeekFrom::End(offset) => VfsSeekFrom::End(offset),
            })
            .map_err(map_seek_error)
    }
}

fn map_file_error(error: VfsError) -> FileError {
    match error {
        VfsError::InvalidInput
        | VfsError::IsDirectory
        | VfsError::NotDirectory
        | VfsError::PermissionDenied
        | VfsError::Unsupported => FileError::BadOperation,
        _ => FileError::Io,
    }
}

fn map_seek_error(error: VfsError) -> SeekError {
    match error {
        VfsError::InvalidInput => SeekError::InvalidOffset,
        VfsError::IsDirectory | VfsError::NotDirectory | VfsError::Unsupported => {
            SeekError::NotSeekable
        }
        _ => SeekError::Io,
    }
}

fn map_file_type(file_type: roxy_vfs::FileType) -> FileType {
    match file_type {
        roxy_vfs::FileType::Regular => FileType::Regular,
        roxy_vfs::FileType::Directory => FileType::Directory,
        roxy_vfs::FileType::Symlink => FileType::Symlink,
        roxy_vfs::FileType::BlockDevice => FileType::BlockDevice,
        roxy_vfs::FileType::CharacterDevice => FileType::CharacterDevice,
        roxy_vfs::FileType::Fifo => FileType::Fifo,
        roxy_vfs::FileType::Socket => FileType::Socket,
        roxy_vfs::FileType::Unknown => FileType::Unknown,
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use alloc::{boxed::Box, sync::Arc, vec::Vec};

    use roxy_vfs::{
        DirEntry, FileHandle, FileSystem, FileType, Metadata, OpenAccess, OpenOptions,
        SeekFrom as VfsSeekFrom, Vfs, VfsError, VfsPath,
    };

    use super::OpenFile;
    use crate::SeekFrom;

    struct MockFileSystem;

    struct Cursor {
        data: Vec<u8>,
        position: usize,
    }

    impl FileSystem for MockFileSystem {
        fn open(
            &self,
            _path: &VfsPath,
            _options: OpenOptions,
        ) -> Result<Box<dyn FileHandle>, VfsError> {
            Ok(Box::new(Cursor {
                data: b"hello".to_vec(),
                position: 0,
            }))
        }

        fn metadata(&self, _path: &VfsPath, _follow: bool) -> Result<Metadata, VfsError> {
            Ok(metadata())
        }

        fn read_dir(&self, _path: &VfsPath) -> Result<Vec<DirEntry>, VfsError> {
            Err(VfsError::Unsupported)
        }

        fn mkdir(&self, _path: &VfsPath) -> Result<(), VfsError> {
            Err(VfsError::Unsupported)
        }

        fn rmdir(&self, _path: &VfsPath) -> Result<(), VfsError> {
            Err(VfsError::Unsupported)
        }

        fn unlink(&self, _path: &VfsPath) -> Result<(), VfsError> {
            Err(VfsError::Unsupported)
        }

        fn hard_link(&self, _source: &VfsPath, _destination: &VfsPath) -> Result<(), VfsError> {
            Err(VfsError::Unsupported)
        }

        fn symlink(&self, _target: &[u8], _link: &VfsPath) -> Result<(), VfsError> {
            Err(VfsError::Unsupported)
        }

        fn read_link(&self, _path: &VfsPath) -> Result<Vec<u8>, VfsError> {
            Err(VfsError::Unsupported)
        }

        fn rename(&self, _source: &VfsPath, _destination: &VfsPath) -> Result<(), VfsError> {
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
            Ok(metadata())
        }

        fn sync(&mut self) -> Result<(), VfsError> {
            Ok(())
        }
    }

    fn metadata() -> Metadata {
        Metadata {
            file_id: 1,
            file_type: FileType::Regular,
            permissions: roxy_vfs::FilePermissions::DEFAULT_FILE,
            size: 5,
            hard_links: 1,
        }
    }

    fn add_offset(base: usize, offset: i64) -> Result<u64, VfsError> {
        let base = u64::try_from(base).map_err(|_| VfsError::InvalidInput)?;

        base.checked_add_signed(offset)
            .ok_or(VfsError::InvalidInput)
    }

    roxy_test::kernel_test!("roxy-fd::vfs-open-file", synchronizes_vfs_position, {
        let vfs = Vfs::new();

        vfs.mount(b"/", Arc::new(MockFileSystem)).unwrap();

        let file = vfs
            .open(
                b"/file",
                OpenOptions {
                    access: OpenAccess::ReadWrite,
                    ..OpenOptions::read_only()
                },
            )
            .unwrap();
        let file = OpenFile::from_vfs(file);
        let mut output = [0; 5];

        assert_eq!(file.read(&mut output[..2]), Ok(2));
        assert_eq!(file.seek(SeekFrom::Current(1)), Ok(3));
        assert_eq!(file.write(b"X"), Ok(1));
        assert_eq!(file.seek(SeekFrom::Start(0)), Ok(0));
        assert_eq!(file.read(&mut output), Ok(5));
        assert_eq!(&output, b"helXo");
        assert_eq!(file.metadata().unwrap().size, 5);
        assert_eq!(file.metadata().unwrap().permissions, 0o644);
    });
}
