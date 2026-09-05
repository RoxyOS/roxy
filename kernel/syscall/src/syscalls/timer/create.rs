use roxy_posix_timer::TimerNotify;

use super::abi::{ClockId, SigEvent, TimerEvent, TimerResult, decode_event, encode_timer_id};
use crate::{
    SyscallResult,
    args::{Nullable, Out},
    numbers::SyscallNumber,
    syscall,
};

syscall!(SyscallNumber::TimerCreate, handle(
    clock: ClockId => Invalid,
    sevp: Nullable<SigEvent> => Fault,
    res: Out<TimerResult> => Fault,
));

fn handle(clock: ClockId, sevp: Nullable<SigEvent>, res: Out<TimerResult>) -> SyscallResult {
    let event = decode_event(sevp.into_option())?;

    let (notify, value) = match event {
        TimerEvent::None => (TimerNotify::None, 0),
        TimerEvent::Signal { signal, value } => (TimerNotify::Signal(signal), value),
    };

    let id = roxy_posix_timer::create(clock.timer_clock(), notify, value)
        .map_err(crate::syscalls::timer::map_error)?;

    // SAFETY: TimerResult has a checked `repr(C)` layout of one integer with no padding, so
    // every byte is initialized by `encode_timer_id`.
    unsafe { res.write(&encode_timer_id(id.as_u32())) }?;

    Ok(0)
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_test::kernel_test;

    use crate::numbers::SyscallNumber;

    kernel_test!(
        "roxy-syscall::timer-create-registered",
        timer_create_registered,
        {
            assert_eq!(SyscallNumber::try_from(73), Ok(SyscallNumber::TimerCreate));
        }
    );
}
