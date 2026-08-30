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

/// An error from duplicating a descriptor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DupError {
    NotOpen,
    NoSpace,
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

    /// Makes `newfd` refer to the same open file description as `oldfd`.
    ///
    /// If `newfd` is already open it is closed first, dropping its previous open file description.
    /// Both descriptors then share the same `Arc<OpenFile>`, so the file position and other
    /// open-file-description state are shared. `close_on_exec` records whether `execve` should
    /// close the new descriptor.
    ///
    /// # Errors
    ///
    /// Returns `DupError::NotOpen` when `oldfd` is not open. `oldfd == newfd` succeeds without
    /// changing anything, matching `dup2` semantics.
    pub fn dup2(&mut self, oldfd: Fd, newfd: Fd, close_on_exec: bool) -> Result<(), DupError> {
        let file = self
            .entries
            .get(&oldfd)
            .ok_or(DupError::NotOpen)?
            .file
            .clone();

        if oldfd == newfd {
            return Ok(());
        }

        self.entries.remove(&newfd);
        self.entries.insert(
            newfd,
            FdEntry {
                file,
                close_on_exec,
            },
        );

        Ok(())
    }

    /// Drops every descriptor marked close-on-exec.
    pub fn drop_close_on_exec(&mut self) {
        self.entries.retain(|_, entry| !entry.close_on_exec);
    }

    /// Returns whether `fd` closes on `exec`.
    ///
    /// # Errors
    ///
    /// Returns `DupError::NotOpen` when `fd` is not open.
    pub fn close_on_exec(&self, fd: Fd) -> Result<bool, DupError> {
        self.entries
            .get(&fd)
            .map(|entry| entry.close_on_exec)
            .ok_or(DupError::NotOpen)
    }

    /// Sets whether `fd` closes on `exec`.
    ///
    /// # Errors
    ///
    /// Returns `DupError::NotOpen` when `fd` is not open.
    pub fn set_close_on_exec(&mut self, fd: Fd, close_on_exec: bool) -> Result<(), DupError> {
        let entry = self.entries.get_mut(&fd).ok_or(DupError::NotOpen)?;
        entry.close_on_exec = close_on_exec;

        Ok(())
    }

    /// Makes the lowest available descriptor at or above `minimum` refer to the same open file
    /// description as `oldfd`, without replacing any lower descriptor.
    ///
    /// This is the `fcntl(F_DUPFD)` and `fcntl(F_DUPFD_CLOEXEC)` semantics, distinct from `dup2`
    /// which targets an exact descriptor. Unlike `dup2`, the new descriptor always differs from
    /// `oldfd`; if `minimum` is already open, the search continues upward. `close_on_exec` records
    /// whether `execve` should close the new descriptor.
    ///
    /// # Errors
    ///
    /// Returns `DupError::NotOpen` when `oldfd` is not open, or `DupError::NoSpace` when every
    /// descriptor at or above `minimum` is occupied.
    pub fn dupfd(&mut self, oldfd: Fd, minimum: Fd, close_on_exec: bool) -> Result<Fd, DupError> {
        let file = self
            .entries
            .get(&oldfd)
            .ok_or(DupError::NotOpen)?
            .file
            .clone();

        let mut value = minimum.as_u32();

        loop {
            let fd = Fd::new(value);

            if let btree_map::Entry::Vacant(entry) = self.entries.entry(fd) {
                entry.insert(FdEntry {
                    file,
                    close_on_exec,
                });
                return Ok(fd);
            }

            value = value.checked_add(1).ok_or(DupError::NoSpace)?;
        }
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

    kernel_test!("roxy-fd::dup2", shares_open_file_reference, {
        let shared = file();
        let mut table = FdTable::new();
        let oldfd = table.insert(shared.clone(), false);

        assert_eq!(table.dup2(oldfd, Fd::new(5), false), Ok(()));
        assert!(Arc::ptr_eq(&table.get(oldfd).unwrap(), &shared));
        assert!(Arc::ptr_eq(&table.get(Fd::new(5)).unwrap(), &shared));
        assert_eq!(Arc::strong_count(&shared), 3);
    });

    kernel_test!("roxy-fd::dup2", replaces_open_descriptor, {
        let old = file();
        let replaced = file();
        let mut table = FdTable::new();
        let oldfd = table.insert(old.clone(), false);
        let occupied = table.insert(replaced.clone(), false);

        assert_eq!(table.dup2(oldfd, occupied, false), Ok(()));
        assert!(Arc::ptr_eq(&table.get(occupied).unwrap(), &old));
        assert_eq!(Arc::strong_count(&replaced), 1);
    });

    kernel_test!("roxy-fd::dup2", rejects_missing_source, {
        let mut table = FdTable::new();

        assert_eq!(
            table.dup2(Fd::new(3), Fd::new(4), false),
            Err(crate::DupError::NotOpen)
        );
    });

    kernel_test!("roxy-fd::dup2", same_descriptor_is_noop, {
        let shared = file();
        let mut table = FdTable::new();
        let fd = table.insert(shared.clone(), false);

        assert_eq!(table.dup2(fd, fd, false), Ok(()));
        assert_eq!(Arc::strong_count(&shared), 2);
    });

    kernel_test!("roxy-fd::dup2", records_close_on_exec, {
        let shared = file();
        let mut table = FdTable::new();
        let oldfd = table.insert(shared, false);
        let newfd = Fd::new(7);

        assert_eq!(table.dup2(oldfd, newfd, true), Ok(()));
        table.drop_close_on_exec();

        assert!(table.get(oldfd).is_some());
        assert!(table.get(newfd).is_none());
    });

    kernel_test!("roxy-fd::close-on-exec", reads_and_writes_the_flag, {
        let mut table = FdTable::new();
        let fd = table.insert(file(), false);

        assert_eq!(table.close_on_exec(fd), Ok(false));
        assert_eq!(table.set_close_on_exec(fd, true), Ok(()));
        assert_eq!(table.close_on_exec(fd), Ok(true));
        assert_eq!(
            table.close_on_exec(Fd::new(9)),
            Err(crate::DupError::NotOpen)
        );
        assert_eq!(
            table.set_close_on_exec(Fd::new(9), true),
            Err(crate::DupError::NotOpen)
        );
    });

    kernel_test!("roxy-fd::dupfd", allocates_at_or_above_minimum, {
        let shared = file();
        let mut table = FdTable::new();
        let oldfd = table.insert(shared.clone(), false);

        assert_eq!(table.dupfd(oldfd, Fd::new(5), false), Ok(Fd::new(5)));
        assert_eq!(table.dupfd(oldfd, Fd::new(5), false), Ok(Fd::new(6)));
        assert_eq!(table.dupfd(oldfd, Fd::new(2), false), Ok(Fd::new(2)));
        assert!(Arc::ptr_eq(&table.get(Fd::new(5)).unwrap(), &shared));
    });

    kernel_test!("roxy-fd::dupfd", rejects_missing_source_fd, {
        let mut table = FdTable::new();

        assert_eq!(
            table.dupfd(Fd::new(3), Fd::new(4), false),
            Err(crate::DupError::NotOpen)
        );
    });

    kernel_test!("roxy-fd::dupfd", records_close_on_exec_flag, {
        let shared = file();
        let mut table = FdTable::new();
        let oldfd = table.insert(shared, false);

        assert_eq!(table.dupfd(oldfd, Fd::new(4), true), Ok(Fd::new(4)));
        table.drop_close_on_exec();

        assert!(table.get(oldfd).is_some());
        assert!(table.get(Fd::new(4)).is_none());
    });
}
