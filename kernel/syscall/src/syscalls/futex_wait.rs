use roxy_futex::FutexError;
use roxy_memory::UserAddress;

use crate::{SyscallResult, errno::Errno, numbers::SyscallNumber, syscall};

syscall!(SyscallNumber::FutexWait, handle(address: UserAddress => Fault, expected: u32 => Invalid, timeout: u64));

fn handle(address: UserAddress, expected: u32, timeout: u64) -> SyscallResult {
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
