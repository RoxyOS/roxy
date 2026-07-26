use core::{mem, time::Duration};

use bitflags::bitflags;
use roxy_fd::{Fd, FileError, PollEvents};
use roxy_memory::UserAddress;

use crate::{
    SyscallResult,
    args::{Slice, SyscallArg},
    errno::Errno,
    numbers::SyscallNumber,
    syscall,
    unsupported::unsupported_argument,
};

syscall!(SyscallNumber::Poll, handle(raw_address: u64, count: usize => Invalid, timeout: i64));

bitflags! {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct PollEventFlags: i16 {
        const IN = 0x0001;
        const PRI = 0x0002;
        const OUT = 0x0004;
        const ERR = 0x0008;
        const HUP = 0x0010;
        const NVAL = 0x0020;
    }
}

// Request for the kernel to poll a fd
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct PollFdAbi {
    // Which fd do you wanna poll
    fd: i32,
    // What event do you wanna poll
    events: i16,
    // What event happened (Filled by kernel)
    revents: i16,
}

const _: () = assert!(mem::size_of::<PollFdAbi>() == 8);

fn handle(raw_address: u64, count: usize, timeout: i64) -> SyscallResult {
    if timeout < -1 {
        return Err(Errno::Invalid);
    }

    if count == 0 {
        return wait_without_descriptors(timeout);
    }

    if timeout != 0 {
        return Err(unsupported_argument(
            "poll.timeout",
            timeout,
            Errno::NotSupported,
        ));
    }

    let address = UserAddress::parse(raw_address, Errno::Fault)?;

    let entries = Slice::<PollFdAbi>::new(address, count);
    entries.validate_writable()?;
    // SAFETY: PollFdAbi's checked repr(C) size equals its fields' combined size, all fields are
    // integers, and every bit pattern is valid.
    let mut values = unsafe { entries.read() }?;

    let mut ready = 0;

    for entry in &mut values {
        entry.revents = poll_entry(*entry);
        if entry.revents != 0 {
            ready += 1;
        }
    }

    // SAFETY: PollFdAbi has no padding and every field in entries is initialized.
    unsafe { entries.write(&values) }?;
    Ok(ready)
}

fn wait_without_descriptors(timeout: i64) -> SyscallResult {
    if timeout == 0 {
        return Ok(0);
    }

    if timeout == -1 {
        return Err(unsupported_argument(
            "poll.timeout-without-fds",
            timeout,
            Errno::NotSupported,
        ));
    }

    let duration = Duration::from_millis(timeout.cast_unsigned());
    let deadline = roxy_time::monotonic_time().saturating_add(duration);

    while roxy_time::monotonic_time() < deadline {
        roxy_thread::scheduler::prepare_block_current_until(deadline).perform();
    }

    Ok(0)
}

fn poll_entry(entry: PollFdAbi) -> i16 {
    if entry.fd < 0 {
        return 0;
    }

    let requested = PollEventFlags::from_bits_truncate(entry.events);
    let fd = Fd::new(entry.fd.cast_unsigned());
    let Ok(file) = roxy_process::current_open_file(fd) else {
        return PollEventFlags::NVAL.bits();
    };

    match file.poll() {
        Ok(events) => encode_events(requested, events),
        Err(FileError::BadOperation) => 0,
        Err(FileError::Io) => PollEventFlags::ERR.bits(),
    }
}

fn encode_events(requested: PollEventFlags, events: PollEvents) -> i16 {
    let mut output = PollEventFlags::empty();

    if requested.contains(PollEventFlags::IN) && events.readable {
        output.insert(PollEventFlags::IN);
    }

    if requested.contains(PollEventFlags::PRI) && events.priority {
        output.insert(PollEventFlags::PRI);
    }

    if requested.contains(PollEventFlags::OUT) && events.writable {
        output.insert(PollEventFlags::OUT);
    }

    if events.error {
        output.insert(PollEventFlags::ERR);
    }

    if events.hangup {
        output.insert(PollEventFlags::HUP);
    }

    output.bits()
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_test::kernel_test;

    use super::{PollEventFlags, encode_events};
    use roxy_fd::PollEvents;

    kernel_test!(
        "roxy-syscall::poll-event-codec",
        converts_supported_event_bits,
        {
            let reported = encode_events(
                PollEventFlags::IN | PollEventFlags::OUT,
                PollEvents {
                    readable: true,
                    writable: true,
                    hangup: true,
                    ..PollEvents::default()
                },
            );
            assert_eq!(
                reported,
                (PollEventFlags::IN | PollEventFlags::OUT | PollEventFlags::HUP).bits()
            );
        }
    );
}
