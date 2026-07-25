use roxy_arch::{Architecture, CurrentArchitectureBackend};
use roxy_memory::UserAddress;

use crate::{SyscallResult, numbers::SyscallNumber, syscall};

syscall!(SyscallNumber::TcbSet, handle(pointer: UserAddress => Invalid));

#[allow(clippy::unnecessary_wraps)]
fn handle(pointer: UserAddress) -> SyscallResult {
    CurrentArchitectureBackend::set_user_thread_pointer(pointer.as_u64());

    Ok(0)
}
