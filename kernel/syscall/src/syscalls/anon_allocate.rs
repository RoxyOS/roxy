use roxy_process::MemoryError;

use crate::{SyscallResult, errno::Errno, numbers::SyscallNumber, syscall};

syscall!(SyscallNumber::AnonAllocate, handle(size: usize => Invalid));

fn handle(size: usize) -> SyscallResult {
    if size == 0 {
        return Err(Errno::Invalid);
    }

    let address = roxy_process::allocate_anonymous(size).map_err(map_memory_error)?;

    Ok(address.as_u64())
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
