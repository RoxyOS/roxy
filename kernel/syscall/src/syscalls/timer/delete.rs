use super::abi::TimerHandle;
use crate::{SyscallResult, numbers::SyscallNumber, syscall};

syscall!(SyscallNumber::TimerDelete, handle(id: TimerHandle => Invalid));

fn handle(id: TimerHandle) -> SyscallResult {
    roxy_posix_timer::delete(id.timer_id()).map_err(crate::syscalls::timer::map_error)?;

    Ok(0)
}
