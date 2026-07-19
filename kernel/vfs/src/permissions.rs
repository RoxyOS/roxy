#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FilePermissions(u16);

impl FilePermissions {
    pub const DEFAULT_FILE: Self = Self(0o644);
    pub const DEFAULT_DIRECTORY: Self = Self(0o755);

    #[must_use]
    pub const fn new(bits: u16) -> Option<Self> {
        if bits & !0o777 == 0 {
            Some(Self(bits))
        } else {
            None
        }
    }

    #[must_use]
    pub const fn bits(self) -> u16 {
        self.0
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use super::FilePermissions;

    roxy_test::kernel_test!("roxy-vfs::file-permissions", validates_permission_bits, {
        assert_eq!(FilePermissions::new(0o640).unwrap().bits(), 0o640);
        assert!(FilePermissions::new(0o1000).is_none());
    });
}
