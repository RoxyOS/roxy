use alloc::{
    collections::{BTreeMap, btree_map},
    sync::Arc,
};

use crate::{Fd, OpenFile};

/// A single descriptor slot: the open file it refers to plus whether it closes on `exec`.
///
/// Close-on-exec is a property of the descriptor, not of the open-file-description, so it lives
/// here rather than on [`OpenFile`].
#[derive(Clone)]
struct FdEntry {
    file: Arc<OpenFile>,
    close_on_exec: bool,
}

#[derive(Clone, Default)]
pub struct FdTable {
    entries: BTreeMap<Fd, FdEntry>,
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
    /// `close_on_exec` records whether `execve` should close the descriptor. `fork` inherits the
    /// flag with the descriptor.
    ///
    /// # Panics
    ///
    /// Panics when every `u32` descriptor number is occupied.
    pub fn insert(&mut self, file: Arc<OpenFile>, close_on_exec: bool) -> Fd {
        let mut value = 0;

        loop {
            let fd = Fd::new(value);

            if let btree_map::Entry::Vacant(entry) = self.entries.entry(fd) {
                entry.insert(FdEntry {
                    file,
                    close_on_exec,
                });
                return fd;
            }

            value = value
                .checked_add(1)
                .expect("file descriptor space exhausted");
        }
    }

    #[must_use]
    pub fn get(&self, fd: Fd) -> Option<Arc<OpenFile>> {
        self.entries.get(&fd).map(|entry| entry.file.clone())
    }

    pub fn remove(&mut self, fd: Fd) -> Option<Arc<OpenFile>> {
        self.entries.remove(&fd).map(|entry| entry.file)
    }

    /// Drops every descriptor marked close-on-exec.
    pub fn drop_close_on_exec(&mut self) {
        self.entries.retain(|_, entry| !entry.close_on_exec);
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

            assert_eq!(table.insert(shared.clone(), false), Fd::new(0));
            assert_eq!(table.insert(shared.clone(), false), Fd::new(1));
            assert!(table.remove(Fd::new(0)).is_some());
            assert!(table.remove(Fd::new(0)).is_none());
            assert_eq!(table.insert(shared, false), Fd::new(0));
        }
    );

    kernel_test!(
        "roxy-fd::clones-open-file-reference",
        get_clones_the_open_file_reference,
        {
            let shared = file();
            let mut table = FdTable::new();
            let fd = table.insert(shared.clone(), false);

            assert!(Arc::ptr_eq(&table.get(fd).unwrap(), &shared));
            assert!(table.get(Fd::new(9)).is_none());
            let removed = table.remove(fd).unwrap();
            assert!(Arc::ptr_eq(&removed, &shared));
            assert!(table.get(fd).is_none());
            drop(removed);
            assert_eq!(Arc::strong_count(&shared), 1);
        }
    );

    kernel_test!(
        "roxy-fd::close-on-exec-drops-only-marked",
        drops_only_the_close_on_exec_descriptor,
        {
            let mut table = FdTable::new();
            let kept = table.insert(file(), false);
            let dropped = table.insert(file(), true);
            let also_kept = table.insert(file(), false);

            table.drop_close_on_exec();

            assert!(table.get(kept).is_some());
            assert!(table.get(also_kept).is_some());
            assert!(table.get(dropped).is_none());
        }
    );

    kernel_test!(
        "roxy-fd::fork-clones-close-on-exec-flag",
        clone_inherits_the_close_on_exec_flag,
        {
            let mut table = FdTable::new();
            let kept = table.insert(file(), false);
            let dropped = table.insert(file(), true);

            let mut child = table.clone();
            child.drop_close_on_exec();

            assert!(child.get(kept).is_some());
            assert!(child.get(dropped).is_none());
        }
    );
}
