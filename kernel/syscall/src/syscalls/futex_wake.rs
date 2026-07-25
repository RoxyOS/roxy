use roxy_futex::FutexError;
use roxy_memory::UserAddress;

use crate::{SyscallResult, errno::Errno, numbers::SyscallNumber, syscall};

syscall!(SyscallNumber::FutexWake, handle(address: UserAddress => Fault, count: usize => Invalid));

fn handle(address: UserAddress, count: usize) -> SyscallResult {
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
