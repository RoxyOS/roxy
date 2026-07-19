use roxy_memory::UserAddress;
use roxy_process::MemoryError;

use super::MemoryProtection;
use crate::{Syscall, SyscallResult, errno::Errno, numbers::SyscallNumber};

pub(super) const SYSCALL: Syscall = Syscall::new(SyscallNumber::VmProtect, handle);

fn handle(arguments: [u64; 6]) -> SyscallResult {
    let address = UserAddress::new(arguments[0]).ok_or(Errno::Invalid)?;
    let size = usize::try_from(arguments[1]).map_err(|_| Errno::Invalid)?;
    let protection = arguments[2];

    if size == 0 {
        return Err(Errno::Invalid);
    }

    let permissions = MemoryProtection::parse_permissions(protection)?;

    roxy_process::protect_memory(address, size, permissions).map_err(map_memory_error)?;

    Ok(0)
}

fn map_memory_error(error: MemoryError) -> Errno {
    match error {
        MemoryError::InvalidRange | MemoryError::PartialUnmap | MemoryError::AddressInUse => {
            Errno::Invalid
        }
        MemoryError::OutOfMemory => Errno::NoMem,
        MemoryError::Fault => Errno::Fault,
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_test::kernel_test;
    use roxy_vm::Permissions;

    use super::MemoryProtection;

    kernel_test!("roxy-syscall::vm-protect-modes", protection_modes, {
        assert_eq!(
            MemoryProtection::parse_permissions(0x1),
            Ok(Permissions::ReadOnly)
        );
        assert_eq!(
            MemoryProtection::parse_permissions(0x3),
            Ok(Permissions::ReadWrite)
        );
        assert_eq!(
            MemoryProtection::parse_permissions(0x5),
            Ok(Permissions::ReadExecute)
        );
    });
}
