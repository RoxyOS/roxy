use alloc::{
    collections::BTreeMap,
    sync::{Arc, Weak},
};
use core::sync::atomic::{AtomicU32, Ordering};

use roxy_devfs::{Device, DynamicDeviceResolver};
use roxy_fd::FileMetadata;
use roxy_utils::Lock;

use crate::pair::{PtyMaster, PtyPair, PtySlave};

/// Names and hands out pty pairs.
///
/// It doubles as the `/dev/ptmx` factory device (each `open` allocates a fresh pair and returns its
/// master) and as the dynamic resolver that maps `/dev/pts/N` to the pair's slave. These roles share
/// one object so kernel-main registers a single hand-off into `roxy-devfs`.
pub struct PtyRegistry {
    next_number: AtomicU32,
    pairs: Lock<BTreeMap<u32, Weak<PtyPair>>>,
}

impl PtyRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self {
            next_number: AtomicU32::new(0),
            pairs: Lock::new(BTreeMap::new()),
        }
    }
    fn allocate(&self) -> Arc<PtyPair> {
        let number = self.next_number.fetch_add(1, Ordering::Relaxed);
        let pair = PtyPair::new(number);
        self.pairs.lock().insert(number, Arc::downgrade(&pair));
        pair
    }

    fn lookup(&self, number: u32) -> Option<Arc<PtyPair>> {
        let mut pairs = self.pairs.lock();

        match pairs.get(&number) {
            None => None,
            Some(weak) if weak.strong_count() == 0 => {
                pairs.remove(&number);
                None
            }
            Some(weak) => weak.upgrade(),
        }
    }

    /// Resolves a mount-relative path such as `pts/3` to its pty pair.
    fn resolve_name(&self, path: &[u8]) -> Option<Arc<PtyPair>> {
        let number = parse_pts_number(path)?;
        self.lookup(number)
    }
}

impl Default for PtyRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl Device for PtyRegistry {
    fn metadata(&self) -> FileMetadata {
        FileMetadata {
            file_id: 3,
            file_type: roxy_fd::FileType::CharacterDevice,
            permissions: 0o666,
            size: 0,
            hard_links: 1,
        }
    }

    /// Opening `/dev/ptmx` allocates a fresh pair and returns its master.
    fn open(&self) -> Option<Arc<dyn Device>> {
        Some(Arc::new(PtyMaster::new(self.allocate())))
    }
}

impl DynamicDeviceResolver for PtyRegistry {
    fn resolve(&self, path: &[u8]) -> Option<Arc<dyn Device>> {
        Some(Arc::new(PtySlave::new(self.resolve_name(path)?)))
    }
}

/// Parses a mount-relative slave path like `pts/0` into the pair number.
fn parse_pts_number(path: &[u8]) -> Option<u32> {
    let mut parts = path.split(|byte| *byte == b'/');

    if parts.next()? != b"pts" {
        return None;
    }

    let name = parts.next()?;
    if parts.next().is_some() {
        return None;
    }

    core::str::from_utf8(name).ok()?.parse().ok()
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_devfs::Device;
    use roxy_test::kernel_test;

    use super::{PtyRegistry, parse_pts_number};

    kernel_test!("roxy-pty::number-parse", parses_pts_paths, {
        assert_eq!(parse_pts_number(b"pts/0"), Some(0));
        assert_eq!(parse_pts_number(b"pts/37"), Some(37));
        assert_eq!(parse_pts_number(b"pts/abc"), None);
        assert_eq!(parse_pts_number(b"ptmx"), None);
        assert_eq!(parse_pts_number(b"pts/0/extra"), None);
    });

    kernel_test!("roxy-pty::ptmx-factory", allocates_unique_masters, {
        let registry = PtyRegistry::new();
        let first = registry.open().unwrap();
        let second = registry.open().unwrap();

        assert_ne!(first.metadata().file_id, second.metadata().file_id);
        assert!(!first.is_terminal());
    });
}
