mod poll;
mod ppoll;
mod pselect;

use alloc::vec::Vec;
use core::{mem, time::Duration};

use bitflags::bitflags;
use roxy_arch::{Architecture, CurrentArchitectureBackend};
use roxy_fd::{Fd, FileError, PollEvents};
use roxy_memory::UserAddress;
use roxy_poll::{PollListener, PollRegistration};

use crate::{
    Syscall, SyscallResult,
    args::{Slice, SyscallArg},
    errno::Errno,
};

pub(super) const POLL_SYSCALL: Syscall = poll::SYSCALL;
pub(super) const PPOLL_SYSCALL: Syscall = ppoll::SYSCALL;
pub(super) const PSELECT_SYSCALL: Syscall = pselect::SYSCALL;

/// Defers `pollfd` pointer validation until `count` is known.
///
/// `poll` and `ppoll` ignore `fds` entirely when `count` is zero.
pub(super) struct PollEntriesAddress(u64);

impl PollEntriesAddress {
    pub(super) fn for_count(self, count: usize) -> Result<UserAddress, Errno> {
        assert_ne!(count, 0);

        UserAddress::parse(self.0, Errno::Fault)
    }
}

impl SyscallArg for PollEntriesAddress {
    fn parse(raw: u64, _error: Errno) -> Result<Self, Errno> {
        Ok(Self(raw))
    }
}

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

fn poll(entries: PollEntriesAddress, count: usize, timeout: Option<Duration>) -> SyscallResult {
    if count == 0 {
        return wait_without_descriptors(timeout);
    }

    let address = entries.for_count(count)?;
    let entries = Slice::<PollFdAbi>::new(address, count);
    entries.validate_writable()?;
    // SAFETY: PollFdAbi's checked repr(C) size equals its fields' combined size, all fields are
    // integers, and every bit pattern is valid.
    let mut values = unsafe { entries.read() }?;

    let ready = poll_until_ready(&mut values, timeout)?;

    // SAFETY: PollFdAbi has no padding and every field in entries is initialized.
    unsafe { entries.write(&values) }?;

    Ok(ready as u64)
}

fn poll_until_ready(values: &mut [PollFdAbi], timeout: Option<Duration>) -> Result<usize, Errno> {
    assert!(!CurrentArchitectureBackend::interrupts_enabled());

    let deadline = timeout.map(|duration| roxy_time::monotonic_time().saturating_add(duration));

    loop {
        if roxy_process::has_pending_signal() {
            return Err(Errno::Interrupted);
        }

        let ready = poll_values(values);

        if ready > 0
            || timeout.is_some_and(|duration| duration.is_zero())
            || deadline.is_some_and(deadline_elapsed)
        {
            return Ok(ready);
        }

        block_until_poll_change(values, deadline);
    }
}

fn deadline_elapsed(deadline: Duration) -> bool {
    roxy_time::monotonic_time() >= deadline
}

fn poll_values(values: &mut [PollFdAbi]) -> usize {
    let mut ready = 0;

    for entry in values {
        entry.revents = poll_entry(*entry);
        ready += usize::from(entry.revents != 0);
    }

    ready
}

fn block_until_poll_change(values: &[PollFdAbi], deadline: Option<Duration>) {
    assert!(!CurrentArchitectureBackend::interrupts_enabled());

    let listener = PollListener::current_thread();
    let registrations = register_poll_listeners(values, &listener);

    if let Some(deadline) = deadline {
        roxy_timer_wait::register_wakeup_deadline(deadline, listener.wait_key());
    }

    let block = roxy_thread::scheduler::prepare_block_current_with_key(listener.wait_key());
    block.perform();

    if deadline.is_some() {
        roxy_timer_wait::cancel_wakeup_deadline(listener.wait_key());
    }

    drop(registrations);
}

fn register_poll_listeners(
    values: &[PollFdAbi],
    listener: &alloc::sync::Arc<PollListener>,
) -> Vec<PollRegistration> {
    let mut registrations = Vec::new();

    for entry in values {
        if entry.fd < 0 {
            continue;
        }

        let Ok(file) = roxy_process::current_open_file(Fd::new(entry.fd.cast_unsigned())) else {
            continue;
        };

        registrations.push(file.register_poll_listener(listener.clone()));
    }

    registrations
}

fn wait_without_descriptors(timeout: Option<Duration>) -> SyscallResult {
    if timeout.is_some_and(|duration| duration.is_zero()) {
        return Ok(0);
    }

    let deadline = timeout.map(|duration| roxy_time::monotonic_time().saturating_add(duration));

    loop {
        if roxy_process::has_pending_signal() {
            return Err(Errno::Interrupted);
        }

        if deadline.is_some_and(deadline_elapsed) {
            return Ok(0);
        }

        match deadline {
            Some(deadline) => roxy_timer_wait::block_current(deadline).perform(),
            None => roxy_thread::scheduler::prepare_block_current().perform(),
        }
    }
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
        Err(FileError::BrokenPipe | FileError::Io) => PollEventFlags::ERR.bits(),
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
    use roxy_fd::PollEvents;
    use roxy_test::kernel_test;

    use super::{PollEventFlags, encode_events};

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
