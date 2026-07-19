use roxy_process::MemoryError;

use crate::{Syscall, SyscallResult, errno::Errno, numbers::SyscallNumber};

pub(super) const SYSCALL: Syscall = Syscall::new(SyscallNumber::AnonAllocate, handle);

fn handle(arguments: [u64; 6]) -> SyscallResult {
    let size = usize::try_from(arguments[0]).map_err(|_| Errno::Invalid)?;

    if size == 0 {
        return Err(Errno::Invalid);
    }

    let address = roxy_process::allocate_anonymous(size).map_err(map_memory_error)?;

    Ok(address.as_u64())
}

fn map_memory_error(error: MemoryError) -> Errno {
    match error {
        MemoryError::InvalidRange => Errno::Invalid,
        MemoryError::OutOfMemory => Errno::NoMem,
        MemoryError::Fault => Errno::Fault,
    }
}
