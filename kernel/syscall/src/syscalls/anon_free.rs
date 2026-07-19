use roxy_memory::UserAddress;
use roxy_process::MemoryError;

use crate::{Syscall, SyscallResult, errno::Errno, numbers::SyscallNumber};

pub(super) const SYSCALL: Syscall = Syscall::new(SyscallNumber::AnonFree, handle);

fn handle(arguments: [u64; 6]) -> SyscallResult {
    let address = UserAddress::new(arguments[0]).ok_or(Errno::Invalid)?;
    let size = usize::try_from(arguments[1]).map_err(|_| Errno::Invalid)?;

    if size == 0 {
        return Err(Errno::Invalid);
    }

    roxy_process::free_anonymous(address, size).map_err(map_memory_error)?;

    Ok(0)
}

fn map_memory_error(error: MemoryError) -> Errno {
    match error {
        MemoryError::InvalidRange => Errno::Invalid,
        MemoryError::OutOfMemory => Errno::NoMem,
        MemoryError::Fault => Errno::Fault,
    }
}
