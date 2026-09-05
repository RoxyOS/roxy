use super::abi::TimerHandle;
use crate::{SyscallResult, args::Out, numbers::SyscallNumber, syscall};

syscall!(SyscallNumber::TimerGetoverrun, handle(
    id: TimerHandle => Invalid,
    out: Out<u32> => Fault,
));

fn handle(id: TimerHandle, out: Out<u32>) -> SyscallResult {
    let overrun =
        roxy_posix_timer::overrun(id.timer_id()).map_err(crate::syscalls::timer::map_error)?;

    // SAFETY: `u32` has a stable layout with every byte initialized.
    unsafe { out.write(&overrun) }?;

    Ok(0)
}
