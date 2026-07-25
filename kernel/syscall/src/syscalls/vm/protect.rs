use roxy_memory::UserAddress;
use roxy_process::MemoryError;

use super::MemoryProtection;
use crate::{SyscallResult, errno::Errno, numbers::SyscallNumber, syscall};

syscall!(SyscallNumber::VmProtect, handle(address: UserAddress => Invalid, size: usize => Invalid, protection: u64));

fn handle(address: UserAddress, size: usize, protection: u64) -> SyscallResult {
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
