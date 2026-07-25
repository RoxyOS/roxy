use alloc::{
    collections::{BTreeMap, btree_map},
    sync::Arc,
};

use crate::{Fd, OpenFile};

#[derive(Clone, Default)]
pub struct FdTable {
    entries: BTreeMap<Fd, Arc<OpenFile>>,
}

impl FdTable {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    /// Inserts an open file at the lowest available descriptor number.
    ///
    /// # Panics
    ///
    /// Panics when every `u32` descriptor number is occupied.
    pub fn insert(&mut self, file: Arc<OpenFile>) -> Fd {
        let mut value = 0;

        loop {
            let fd = Fd::new(value);

            if let btree_map::Entry::Vacant(entry) = self.entries.entry(fd) {
                entry.insert(file);
                return fd;
            }

            value = value
                .checked_add(1)
                .expect("file descriptor space exhausted");
        }
    }

    #[must_use]
    pub fn get(&self, fd: Fd) -> Option<Arc<OpenFile>> {
        self.entries.get(&fd).cloned()
    }

    pub fn remove(&mut self, fd: Fd) -> Option<Arc<OpenFile>> {
        self.entries.remove(&fd)
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use alloc::{boxed::Box, sync::Arc};

    use super::FdTable;
    use crate::{Fd, File, FileError, OpenFile, PollEvents, SeekError, SeekFrom};
    use roxy_test::kernel_test;

    struct Unsupported;

    impl File for Unsupported {
        fn poll(&mut self) -> Result<PollEvents, FileError> {
            Ok(PollEvents::default())
        }

        fn is_terminal(&self) -> bool {
            false
        }

        fn metadata(&self) -> Result<crate::FileMetadata, FileError> {
            Err(FileError::BadOperation)
        }

        fn read(&mut self, _position: &mut u64, _output: &mut [u8]) -> Result<usize, FileError> {
            Err(FileError::BadOperation)
        }

        fn write(&mut self, _position: &mut u64, _input: &[u8]) -> Result<usize, FileError> {
            Err(FileError::BadOperation)
        }

        fn seek(&mut self, _current: u64, _position: SeekFrom) -> Result<u64, SeekError> {
            Err(SeekError::NotSeekable)
        }
    }

    fn file() -> Arc<OpenFile> {
        OpenFile::new(Box::new(Unsupported))
    }

    kernel_test!(
        "roxy-fd::reuses-lowest-descriptor",
        reuses_lowest_available_descriptor,
        {
            let shared = file();
            let mut table = FdTable::new();

            assert_eq!(table.insert(shared.clone()), Fd::new(0));
            assert_eq!(table.insert(shared.clone()), Fd::new(1));
            assert!(table.remove(Fd::new(0)).is_some());
            assert!(table.remove(Fd::new(0)).is_none());
            assert_eq!(table.insert(shared), Fd::new(0));
        }
    );

    kernel_test!(
        "roxy-fd::clones-open-file-reference",
        get_clones_the_open_file_reference,
        {
            let shared = file();
            let mut table = FdTable::new();
            let fd = table.insert(shared.clone());

            assert!(Arc::ptr_eq(&table.get(fd).unwrap(), &shared));
            assert!(table.get(Fd::new(9)).is_none());
            let removed = table.remove(fd).unwrap();
            assert!(Arc::ptr_eq(&removed, &shared));
            assert!(table.get(fd).is_none());
            drop(removed);
            assert_eq!(Arc::strong_count(&shared), 1);
        }
    );
}
