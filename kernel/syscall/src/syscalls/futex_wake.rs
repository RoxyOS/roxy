use roxy_futex::FutexError;
use roxy_memory::UserAddress;

use crate::{Syscall, SyscallResult, errno::Errno, numbers::SyscallNumber};

pub(super) const SYSCALL: Syscall = Syscall::new(SyscallNumber::FutexWake, handle);

fn handle(arguments: [u64; 6]) -> SyscallResult {
    let address = UserAddress::new(arguments[0]).ok_or(Errno::Fault)?;
    let count = usize::try_from(arguments[1]).map_err(|_| Errno::Invalid)?;

    if !address.as_u64().is_multiple_of(4) {
        return Err(Errno::Invalid);
    }

    let addrspace = roxy_process::current_addrspace().map_err(|_| Errno::Fault)?;
    let woken = roxy_futex::wake(&addrspace, address, count).map_err(map_futex_error)?;

    Ok(u64::try_from(woken).unwrap())
}

fn map_futex_error(error: FutexError) -> Errno {
    match error {
        FutexError::Fault => Errno::Fault,
        FutexError::Invalid => Errno::Invalid,
        FutexError::Mismatch => unreachable!("wake does not compare user memory"),
    }
}
