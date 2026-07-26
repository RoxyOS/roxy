use core::{mem, time::Duration};

use roxy_memory::UserAddress;

use crate::{
    SyscallResult,
    args::{SyscallArg, user_memory},
    errno::Errno,
    numbers::SyscallNumber,
    syscall,
};

syscall!(SyscallNumber::Sleep, handle(request: SleepRequest => Fault));

const NANOS_PER_SECOND: i64 = 1_000_000_000;

#[repr(C)]
#[derive(Clone, Copy)]
struct TimespecAbi {
    seconds: i64,
    nanoseconds: i64,
}

const _: () = assert!(mem::size_of::<TimespecAbi>() == 16);

#[derive(Clone, Copy)]
struct SleepRequest(Duration);

impl SyscallArg for SleepRequest {
    fn parse(raw: u64, error: Errno) -> Result<Self, Errno> {
        let address = UserAddress::parse(raw, error)?;
        let mut timespec = TimespecAbi {
            seconds: 0,
            nanoseconds: 0,
        };

        // SAFETY: TimespecAbi checked repr(C) layout contains only initialized i64 fields, so
        // every copied byte pattern is valid.
        unsafe { user_memory::read(address, &mut timespec) }?;

        decode(timespec).map(Self)
    }
}

#[allow(clippy::unnecessary_wraps)]
fn handle(request: SleepRequest) -> SyscallResult {
    let deadline = roxy_time::monotonic_time().saturating_add(request.0);

    if deadline > roxy_time::monotonic_time() {
        roxy_timer_wait::block_current(deadline).perform();
    }

    Ok(0)
}

fn decode(timespec: TimespecAbi) -> Result<Duration, Errno> {
    if timespec.seconds < 0 || !(0..NANOS_PER_SECOND).contains(&timespec.nanoseconds) {
        return Err(Errno::Invalid);
    }

    Ok(Duration::new(
        timespec.seconds.cast_unsigned(),
        timespec.nanoseconds.cast_unsigned().try_into().unwrap(),
    ))
}

#[cfg(feature = "kernel-test")]
mod tests {
    use core::time::Duration;

    use roxy_test::kernel_test;

    use super::{TimespecAbi, decode};
    use crate::errno::Errno;

    kernel_test!("roxy-syscall::sleep-timespec", validates_timespec, {
        assert_eq!(
            decode(TimespecAbi {
                seconds: 2,
                nanoseconds: 3,
            }),
            Ok(Duration::new(2, 3))
        );
        assert_eq!(
            decode(TimespecAbi {
                seconds: -1,
                nanoseconds: 0,
            }),
            Err(Errno::Invalid)
        );
        assert_eq!(
            decode(TimespecAbi {
                seconds: 0,
                nanoseconds: 1_000_000_000,
            }),
            Err(Errno::Invalid)
        );
    });
}
