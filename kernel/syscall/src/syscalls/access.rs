use alloc::vec::Vec;
use roxy_vfs::AccessMode;

use crate::{
    SyscallResult,
    args::{CString, SyscallArg},
    errno::Errno,
    numbers::SyscallNumber,
    syscall,
};

impl SyscallArg for Vec<AccessMode> {
    fn parse(raw: u64, error: Errno) -> Result<Self, Errno> {
        let bits = u8::try_from(raw).map_err(|_| error)?;
        if bits & !0b111 != 0 {
            return Err(unsupported("access.mode", u64::from(bits), error));
        }

        let mut mode = Vec::new();
        if bits & 0b100 != 0 {
            mode.push(AccessMode::Read);
        }
        if bits & 0b010 != 0 {
            mode.push(AccessMode::Write);
        }
        if bits & 0b001 != 0 {
            mode.push(AccessMode::Execute);
        }

        Ok(mode)
    }
}

syscall!(SyscallNumber::Access, handle(
    path: CString => Fault,
    mode: Vec<AccessMode> => Invalid,
));

// The `Vec<AccessMode>` signature is fixed by the `syscall!` macro's `SyscallArg` plumbing even
// though the handler only borrows the list to call `Vfs::access`.
#[allow(clippy::needless_pass_by_value)]
fn handle(path: CString, mode: Vec<AccessMode>) -> SyscallResult {
    if path.is_empty() {
        return Err(Errno::NotFound);
    }

    roxy_vfs::access(path.into_inner(), &mode).map_err(map_vfs_error)?;

    Ok(0)
}

fn map_vfs_error(error: roxy_vfs::VfsError) -> Errno {
    match error {
        roxy_vfs::VfsError::NotFound | roxy_vfs::VfsError::InvalidPath => Errno::NotFound,
        roxy_vfs::VfsError::PermissionDenied => Errno::Access,
        _ => Errno::Io,
    }
}

fn unsupported(operation: &str, argument: u64, errno: Errno) -> Errno {
    crate::unsupported::unsupported_argument(operation, argument, errno)
}
