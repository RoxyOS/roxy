//! Waiting for and consuming pending signals.

use roxy_signal::SignalSet;
use roxy_thread::scheduler;

use crate::{
    signal_frame::{self, Siginfo},
    table::PROCESS_TABLE,
};

/// Reports whether the current thread has a pending signal that its mask permits.
#[must_use]
pub fn has_pending_signal() -> bool {
    let mut table = PROCESS_TABLE.lock();
    let Some(thread_id) = scheduler::try_current_thread_id() else {
        return false;
    };
    let Some(process) = table.current_process() else {
        return false;
    };

    process.has_pending_for(thread_id)
}

/// Reports whether the current thread has any pending signal that its mask does **not** block.
///
/// A pending unmasked signal is delivered at the next userspace return boundary; this reports
/// whether such a delivery is imminent for the current thread.
#[must_use]
pub fn has_unmasked_pending_signal() -> bool {
    let mut table = PROCESS_TABLE.lock();
    let Some(thread_id) = scheduler::try_current_thread_id() else {
        return false;
    };
    let Some(process) = table.current_process() else {
        return false;
    };

    process.has_pending_for(thread_id)
}

/// A consumed pending signal, returned to a user: the signal number plus its information
/// record.
pub struct SigWait {
    /// The consumed signal's number.
    pub number: i32,
    /// The information record for the consumed signal.
    pub info: Siginfo,
}

/// Consumes the most recent pending signal of the current thread whose number is in `set`,
/// whether or not the thread currently blocks it.
///
/// Scans the thread's targeted queue first, then the process queue. Consuming a blocked signal
/// lets a thread retrieve a signal it has masked instead of waiting for it to become unmasked.
#[must_use]
pub fn take_matching_pending_signal(set: SignalSet) -> Option<SigWait> {
    let mut table = PROCESS_TABLE.lock();
    let thread_id = scheduler::current_thread_id();
    let process = table.current_process()?;

    process
        .take_matching(thread_id, set)
        .map(|pending| SigWait {
            number: i32::from(pending.signal.number()),
            info: signal_frame::build_siginfo(pending),
        })
}
