use super::abi::{Itimerspec, TimerHandle, deadline_from, is_absolute};
use crate::{
    SyscallResult,
    args::{Nullable, Out, Timespec},
    numbers::SyscallNumber,
    syscall,
};

syscall!(SyscallNumber::TimerSettime, handle(
    id: TimerHandle => Invalid,
    flags: u32 => Invalid,
    val: Itimerspec => Fault,
    old: Nullable<Out<Itimerspec>> => Fault,
));

fn handle(
    id: TimerHandle,
    flags: u32,
    val: Itimerspec,
    old: Nullable<Out<Itimerspec>>,
) -> SyscallResult {
    let absolute = is_absolute(flags)?;
    let timer = id.timer_id();

    // Capture the current configuration before re-arming, for the optional `old` output.
    if let Some(old) = old.into_option() {
        let (interval, remaining) =
            roxy_posix_timer::current(timer).map_err(crate::syscalls::timer::map_error)?;
        let old_value = Itimerspec {
            interval: Timespec::from_duration(interval),
            value: Timespec::from_duration(remaining),
        };

        // SAFETY: Itimerspec has a checked `repr(C)` layout and every field is initialized.
        unsafe { old.write(&old_value) }?;
    }

    let (interval, value) = val.durations();

    // A zero `it_value` disarms the timer (POSIX), whether relative or absolute.
    if value.is_zero() {
        roxy_posix_timer::disarm(timer).map_err(crate::syscalls::timer::map_error)?;
        return Ok(0);
    }

    let clock = roxy_posix_timer::clock(timer).map_err(crate::syscalls::timer::map_error)?;
    let deadline = deadline_from(clock, value, absolute);

    // An absolute deadline already in the past fires on the next tick.
    roxy_posix_timer::arm(timer, deadline, interval).map_err(crate::syscalls::timer::map_error)?;

    Ok(0)
}
