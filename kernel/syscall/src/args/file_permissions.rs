use roxy_vfs::FilePermissions;

use super::SyscallArg;
use crate::errno::Errno;

impl SyscallArg for FilePermissions {
    fn parse(raw: u64, error: Errno) -> Result<Self, Errno> {
        let bits = u16::try_from(raw).map_err(|_| error)?;

        Self::new(bits).ok_or(error)
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_test::kernel_test;
    use roxy_vfs::FilePermissions;

    use super::SyscallArg;
    use crate::errno::Errno;

    kernel_test!(
        "roxy-syscall::file-permissions-argument",
        parses_permissions,
        {
            assert_eq!(
                FilePermissions::parse(0o750, Errno::Invalid),
                Ok(FilePermissions::new(0o750).unwrap())
            );
            assert_eq!(
                FilePermissions::parse(0o1777, Errno::Invalid),
                Ok(FilePermissions::new(0o1777).unwrap())
            );
            assert_eq!(
                FilePermissions::parse(0o10000, Errno::Invalid),
                Err(Errno::Invalid)
            );
        }
    );
}
