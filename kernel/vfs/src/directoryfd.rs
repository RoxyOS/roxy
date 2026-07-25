use alloc::vec::Vec;

use roxy_fd::{
    Directory as FdDirectory, DirectoryEntry as FdDirectoryEntry, File as FdFile,
    FileError as FdFileError, FileMetadata as FdFileMetadata, FileType as FdFileType, PollEvents,
    SeekError as FdSeekError, SeekFrom as FdSeekFrom,
};

use crate::VfsDirectory;

impl FdFile for VfsDirectory {
    fn poll(&mut self) -> Result<PollEvents, FdFileError> {
        Ok(PollEvents {
            readable: true,
            writable: true,
            ..PollEvents::default()
        })
    }

    fn is_terminal(&self) -> bool {
        false
    }

    fn metadata(&self) -> Result<FdFileMetadata, FdFileError> {
        let metadata = self.metadata();

        Ok(FdFileMetadata {
            file_id: metadata.file_id,
            file_type: map_file_type(metadata.file_type),
            permissions: metadata.permissions.bits(),
            size: metadata.size,
            hard_links: metadata.hard_links,
        })
    }

    fn read(&mut self, _position: &mut u64, _output: &mut [u8]) -> Result<usize, FdFileError> {
        Err(FdFileError::BadOperation)
    }

    fn write(&mut self, _position: &mut u64, _input: &[u8]) -> Result<usize, FdFileError> {
        Err(FdFileError::BadOperation)
    }

    fn seek(&mut self, current: u64, position: FdSeekFrom) -> Result<u64, FdSeekError> {
        // Directory positions are entry indices rather than byte offsets.
        let length = u64::try_from(self.entries().len()).map_err(|_| FdSeekError::Overflow)?;
        let position = directory_position(current, length, position)?;

        if position > length {
            return Err(FdSeekError::InvalidOffset);
        }

        Ok(position)
    }

    fn as_directory(&mut self) -> Option<&mut dyn FdDirectory> {
        Some(self)
    }
}

impl FdDirectory for VfsDirectory {
    fn read_entries(
        &mut self,
        position: &mut u64,
        limit: usize,
    ) -> Result<Vec<FdDirectoryEntry>, FdFileError> {
        read_entries_inner(self.entries(), position, limit)
    }
}

fn read_entries_inner(
    entries: &[crate::DirEntry],
    position: &mut u64,
    limit: usize,
) -> Result<Vec<FdDirectoryEntry>, FdFileError> {
    let start = usize::try_from(*position).map_err(|_| FdFileError::Io)?;
    let end = start.saturating_add(limit).min(entries.len());
    let output = entries[start.min(entries.len())..end]
        .iter()
        .enumerate()
        .map(|(index, entry)| FdDirectoryEntry {
            file_id: entry.file_id,
            offset: u64::try_from(start + index + 1).unwrap(),
            file_type: map_file_type(entry.file_type),
            name: entry.name.clone(),
        })
        .collect();

    *position = u64::try_from(end).map_err(|_| FdFileError::Io)?;

    Ok(output)
}

fn directory_position(current: u64, length: u64, position: FdSeekFrom) -> Result<u64, FdSeekError> {
    match position {
        FdSeekFrom::Start(position) => Ok(position),
        FdSeekFrom::Current(offset) => current
            .checked_add_signed(offset)
            .ok_or(FdSeekError::InvalidOffset),
        FdSeekFrom::End(offset) => length
            .checked_add_signed(offset)
            .ok_or(FdSeekError::InvalidOffset),
    }
}

const fn map_file_type(file_type: crate::FileType) -> FdFileType {
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
    use alloc::vec;
    use roxy_test::kernel_test;

    use super::{directory_position, read_entries_inner};
    use crate::DirEntry;
    use roxy_fd::{FileType as FdFileType, SeekError as FdSeekError, SeekFrom as FdSeekFrom};

    kernel_test!(
        "roxy-vfs::directory-batches",
        advances_directory_position,
        {
            let entries = vec![
                DirEntry {
                    file_id: 10,
                    name: vec![b'a'],
                    file_type: crate::FileType::Regular,
                },
                DirEntry {
                    file_id: 11,
                    name: vec![b'b'],
                    file_type: crate::FileType::Directory,
                },
            ];
            let mut position = 0;

            let first = read_entries_inner(&entries, &mut position, 1).unwrap();
            assert_eq!(first[0].file_id, 10);
            assert_eq!(first[0].offset, 1);
            assert_eq!(first[0].file_type, FdFileType::Regular);
            assert_eq!(position, 1);

            let second = read_entries_inner(&entries, &mut position, 8).unwrap();
            assert_eq!(second[0].offset, 2);
            assert_eq!(second[0].file_type, FdFileType::Directory);
            assert_eq!(position, 2);
            assert!(
                read_entries_inner(&entries, &mut position, 8)
                    .unwrap()
                    .is_empty()
            );
        }
    );

    kernel_test!(
        "roxy-vfs::directory-position",
        validates_directory_position,
        {
            assert_eq!(directory_position(2, 5, FdSeekFrom::Start(0)), Ok(0));
            assert_eq!(directory_position(2, 5, FdSeekFrom::Current(2)), Ok(4));
            assert_eq!(directory_position(2, 5, FdSeekFrom::End(-1)), Ok(4));
            assert_eq!(
                directory_position(0, 5, FdSeekFrom::Current(-1)),
                Err(FdSeekError::InvalidOffset)
            );
        }
    );
}
