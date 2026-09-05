use super::abi::{Itimerspec, TimerHandle};
use crate::{
    SyscallResult,
    args::{Out, Timespec},
    numbers::SyscallNumber,
    syscall,
};

syscall!(SyscallNumber::TimerGettime, handle(
    id: TimerHandle => Invalid,
    val: Out<Itimerspec> => Fault,
));

fn handle(id: TimerHandle, val: Out<Itimerspec>) -> SyscallResult {
    let (interval, remaining) =
        roxy_posix_timer::current(id.timer_id()).map_err(crate::syscalls::timer::map_error)?;

    let current = Itimerspec {
        interval: Timespec::from_duration(interval),
        value: Timespec::from_duration(remaining),
    };

    // SAFETY: Itimerspec has a checked `repr(C)` layout and every field is initialized.
    unsafe { val.write(&current) }?;

    Ok(0)
}
