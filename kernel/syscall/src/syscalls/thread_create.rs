use roxy_memory::UserAddress;
use roxy_process::ThreadCreateError;

use crate::{SyscallResult, errno::Errno, numbers::SyscallNumber, syscall};

syscall!(
    SyscallNumber::ThreadCreate,
    handle(entry: UserAddress => Fault, stack: UserAddress => Fault)
);

fn handle(entry: UserAddress, stack: UserAddress) -> SyscallResult {
    let tid = roxy_process::create_thread(entry, stack).map_err(map_thread_create_error)?;
    Ok(tid.as_u64())
}

fn map_thread_create_error(error: ThreadCreateError) -> Errno {
    match error {
        ThreadCreateError::OutOfMemory => Errno::NoMem,
    }
}
