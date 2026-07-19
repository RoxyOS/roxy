use roxy_arch::{Architecture, CurrentArchitectureBackend};
use roxy_memory::UserAddress;

use crate::{Syscall, SyscallResult, errno::Errno, numbers::SyscallNumber};

pub(super) const SYSCALL: Syscall = Syscall::new(SyscallNumber::TcbSet, handle);

fn handle(arguments: [u64; 6]) -> SyscallResult {
    let pointer = UserAddress::new(arguments[0]).ok_or(Errno::Invalid)?;

    CurrentArchitectureBackend::set_user_thread_pointer(pointer.as_u64());

    Ok(0)
}
