#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FilePermissions(u16);

impl FilePermissions {
    pub const DEFAULT_FILE: Self = Self(0o644);
    pub const DEFAULT_DIRECTORY: Self = Self(0o755);
    pub const DEFAULT_UMASK: Self = Self(0o022);

    #[must_use]
    pub const fn new(bits: u16) -> Option<Self> {
        if bits & !0o7777 == 0 {
            Some(Self(bits))
        } else {
            None
        }
    }

    #[must_use]
    pub const fn bits(self) -> u16 {
        self.0
    }

    /// Clears the permission bits set in `mask`, implementing a file mode creation mask.
    ///
    /// The mask is applied to the mode passed to `open`/`mkdir`: every bit set in `mask` is
    /// cleared in the resulting permissions, so a mask can only remove, never add, permissions.
    #[must_use]
    pub const fn apply_umask(self, mask: FilePermissions) -> FilePermissions {
        Self(self.0 & !mask.0)
    }
}

/// One access right checked by `access(2)` / `faccessat(2)`.
///
/// The requested mode is expressed as a list of these rights; an empty list tests only for
/// existence (`F_OK`). Values are intentionally ABI-neutral — raw bit decoding happens in the
/// syscall layer, not here.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccessMode {
    /// Test for read permission (`R_OK`).
    Read,
    /// Test for write permission (`W_OK`).
    Write,
    /// Test for execute permission (`X_OK`).
    Execute,
}

#[cfg(feature = "kernel-test")]
mod tests {
    use super::FilePermissions;

    roxy_test::kernel_test!("roxy-vfs::file-permissions", validates_permission_bits, {
        assert_eq!(FilePermissions::new(0o640).unwrap().bits(), 0o640);
        assert_eq!(FilePermissions::new(0o1777).unwrap().bits(), 0o1777);
        assert!(FilePermissions::new(0o10000).is_none());
    });

    roxy_test::kernel_test!("roxy-vfs::file-permissions", applies_umask_mask, {
        let full = FilePermissions::new(0o666).unwrap();
        let mask = FilePermissions::new(0o022).unwrap();

        assert_eq!(full.apply_umask(mask).bits(), 0o644);

        let everything = FilePermissions::new(0o777).unwrap();
        let private = FilePermissions::new(0o077).unwrap();
        assert_eq!(everything.apply_umask(private).bits(), 0o700);
    });
}
