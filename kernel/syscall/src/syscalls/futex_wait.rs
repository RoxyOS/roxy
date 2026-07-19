use roxy_futex::FutexError;
use roxy_memory::UserAddress;

use crate::{Syscall, SyscallResult, errno::Errno, numbers::SyscallNumber};

pub(super) const SYSCALL: Syscall = Syscall::new(SyscallNumber::FutexWait, handle);

fn handle(arguments: [u64; 6]) -> SyscallResult {
    let address = UserAddress::new(arguments[0]).ok_or(Errno::Fault)?;
    let expected = u32::try_from(arguments[1]).map_err(|_| Errno::Invalid)?;
    let timeout = arguments[2];

    if !address.as_u64().is_multiple_of(4) {
        return Err(Errno::Invalid);
    }

    if timeout != 0 {
        return Err(Errno::NotSupported);
    }

    let addrspace = roxy_process::current_addrspace().map_err(|_| Errno::Fault)?;
    roxy_futex::wait(&addrspace, address, expected).map_err(map_futex_error)?;

    Ok(0)
}

fn map_futex_error(error: FutexError) -> Errno {
    match error {
        FutexError::Fault => Errno::Fault,
        FutexError::Invalid => Errno::Invalid,
        FutexError::Mismatch => Errno::Again,
    }
}
