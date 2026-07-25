use roxy_process::MemoryError;

use roxy_memory::UserAddress;

use crate::{SyscallResult, errno::Errno, numbers::SyscallNumber, syscall};

syscall!(SyscallNumber::AnonFree, handle(address: UserAddress => Invalid, size: usize => Invalid));

fn handle(address: UserAddress, size: usize) -> SyscallResult {
    if size == 0 {
        return Err(Errno::Invalid);
    }

    roxy_process::free_anonymous(address, size).map_err(map_memory_error)?;

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
