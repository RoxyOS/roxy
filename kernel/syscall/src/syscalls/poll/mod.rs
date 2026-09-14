mod abi;
// The `Poll` syscall shim lives in a sibling file named `poll.rs`, so this module shares its
// parent module's name (`crate::syscalls::poll::poll`). That keeps each syscall shim in its own
// file; the inception is intentional.
#[allow(clippy::module_inception)]
mod poll;
mod ppoll;

use alloc::vec::Vec;
use core::time::Duration;

use roxy_arch::{Architecture, CurrentArchitectureBackend};
use roxy_fd::{Fd, FileError};
use roxy_memory::UserAddress;
use roxy_poll::{PollListener, PollRegistration};

use self::abi::{PollReportedEvents, PollRequestAbi, PollRequestedEvents, reported_events};
use crate::{
    Syscall, SyscallResult,
    args::{Slice, SyscallArg},
    errno::Errno,
};

pub(super) const POLL_SYSCALL: Syscall = poll::SYSCALL;
pub(super) const PPOLL_SYSCALL: Syscall = ppoll::SYSCALL;

/// Defers request-array validation until `count` is known.
///
/// `poll` and `ppoll` ignore `requests` entirely when `count` is zero.
pub(super) struct PollRequestsAddress(u64);

impl PollRequestsAddress {
    pub(super) fn for_count(self, count: usize) -> Result<UserAddress, Errno> {
        assert_ne!(count, 0);

        UserAddress::parse(self.0, Errno::Fault)
    }
}

impl SyscallArg for PollRequestsAddress {
    fn parse(raw: u64, _error: Errno) -> Result<Self, Errno> {
        Ok(Self(raw))
    }
}

fn poll(requests: PollRequestsAddress, count: usize, timeout: Option<Duration>) -> SyscallResult {
    if count == 0 {
        return wait_without_descriptors(timeout);
    }

    let address = requests.for_count(count)?;
    let requests = Slice::<PollRequestAbi>::new(address, count);
    requests.validate_writable()?;
    // SAFETY: PollRequestAbi's checked repr(C) size equals its fields' combined size, all fields
    // are integers, and every bit pattern is valid.
    let mut values = unsafe { requests.read() }?;
    let requested = parse_requested_events(&values)?;

    let ready = poll_until_ready(&mut values, &requested, timeout)?;

    // SAFETY: PollRequestAbi has no padding and every field in requests is initialized.
    unsafe { requests.write(&values) }?;

    Ok(ready as u64)
}

/// Decodes every entry's request word before any readiness is queried.
///
/// A request that names a condition this kernel does not serve is reported here rather than
/// waited for, because waiting on it could never end. The reserve is fallible because the entry
/// count is userspace's.
fn parse_requested_events(values: &[PollRequestAbi]) -> Result<Vec<PollRequestedEvents>, Errno> {
    let mut requested = Vec::new();
    requested
        .try_reserve_exact(values.len())
        .map_err(|_| Errno::NoMem)?;

    for value in values {
        requested.push(PollRequestedEvents::from_record(value.requested_events)?);
    }

    Ok(requested)
}

fn poll_until_ready(
    values: &mut [PollRequestAbi],
    requested: &[PollRequestedEvents],
    timeout: Option<Duration>,
) -> Result<usize, Errno> {
    assert!(!CurrentArchitectureBackend::interrupts_enabled());

    let deadline = timeout.map(|duration| roxy_time::monotonic_time().saturating_add(duration));

    loop {
        if roxy_process::has_pending_signal() {
            return Err(Errno::Interrupted);
        }

        let ready = poll_values(values, requested);

        if ready > 0
            || timeout.is_some_and(|duration| duration.is_zero())
            || deadline.is_some_and(deadline_elapsed)
        {
            return Ok(ready);
        }

        block_until_poll_change(values, requested, deadline);
    }
}

fn deadline_elapsed(deadline: Duration) -> bool {
    roxy_time::monotonic_time() >= deadline
}

fn poll_values(values: &mut [PollRequestAbi], requested: &[PollRequestedEvents]) -> usize {
    let mut ready = 0;

    for (entry, requested) in values.iter_mut().zip(requested) {
        entry.reported_events = poll_entry(*entry, *requested).bits();
        ready += usize::from(entry.reported_events != 0);
    }

    ready
}

fn block_until_poll_change(
    values: &mut [PollRequestAbi],
    requested: &[PollRequestedEvents],
    deadline: Option<Duration>,
) {
    assert!(!CurrentArchitectureBackend::interrupts_enabled());

    // Register the wake listener with every source BEFORE re-checking readiness, and block with a
    // wake latch (see `prepare_block_current_with_key_and_latch`). Together these close the SMP
    // lost-wakeup windows: a source that becomes ready during registration is caught either by the
    // re-scan below or, when it changes after the re-scan, by a notification through the now-
    // registered listener that is recorded in the latch instead of dropped for a not-yet-blocked
    // thread.
    let listener = PollListener::current_thread();
    let registrations = register_poll_listeners(values, &listener);

    if let Some(deadline) = deadline {
        roxy_timer_wait::register_wakeup_deadline(deadline, listener.wait_key());
    }

    // Now that every listener is registered, re-scan readiness. A source that became ready during
    // registration would never notify this listener (it was not registered yet), so return and let
    // the outer loop report the ready set instead of sleeping forever.
    if poll_values(values, requested) > 0 {
        if deadline.is_some() {
            roxy_timer_wait::cancel_wakeup_deadline(listener.wait_key());
        }
        drop(registrations);
        return;
    }

    let block = roxy_thread::scheduler::prepare_block_current_with_key_and_latch(
        listener.wait_key(),
        listener.notified(),
    );
    block.perform();

    if deadline.is_some() {
        roxy_timer_wait::cancel_wakeup_deadline(listener.wait_key());
    }

    drop(registrations);
}

fn register_poll_listeners(
    values: &[PollRequestAbi],
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

fn poll_entry(entry: PollRequestAbi, requested: PollRequestedEvents) -> PollReportedEvents {
    if entry.fd < 0 {
        return PollReportedEvents::empty();
    }

    let fd = Fd::new(entry.fd.cast_unsigned());
    let Ok(file) = roxy_process::current_open_file(fd) else {
        return PollReportedEvents::INVALID_DESCRIPTOR;
    };

    match file.poll() {
        Ok(events) => reported_events(requested, events),
        Err(FileError::BadOperation | FileError::WouldBlock) => PollReportedEvents::empty(),
        Err(
            FileError::BrokenPipe
            | FileError::NotConnected
            | FileError::Io
            | FileError::Interrupted,
        ) => PollReportedEvents::ERROR,
    }
}
