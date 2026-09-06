use core::sync::atomic::{AtomicUsize, Ordering};

use super::state::{Scheduler, ThreadState};
use crate::ThreadId;

static REAPED_HANDLER: AtomicUsize = AtomicUsize::new(0);

pub type ThreadReapedHandler = fn(ThreadId);
pub type ThreadExitHandler = fn(ThreadId);

static EXIT_HANDLER: AtomicUsize = AtomicUsize::new(0);

/// Registers the owner notification invoked after a thread is safely reaped.
///
/// # Panics
///
/// Panics when a handler was already registered.
pub fn register_reaped_handler(handler: ThreadReapedHandler) {
    assert_eq!(
        REAPED_HANDLER.swap(handler as usize, Ordering::AcqRel),
        0,
        "thread reaped handler registered twice"
    );
}

/// Registers the owner notification invoked before a thread leaves the scheduler.
///
/// # Panics
///
/// Panics when a handler was already registered.
pub fn register_exit_handler(handler: ThreadExitHandler) {
    assert_eq!(
        EXIT_HANDLER.swap(handler as usize, Ordering::AcqRel),
        0,
        "thread exit handler registered twice"
    );
}

pub(super) fn notify_exit(exiting: Option<ThreadId>) {
    let Some(thread_id) = exiting else {
        return;
    };
    let address = EXIT_HANDLER.load(Ordering::Acquire);

    if address == 0 {
        return;
    }

    // SAFETY: register_exit_handler stores one permanent ThreadExitHandler pointer.
    let handler: ThreadExitHandler = unsafe { core::mem::transmute(address) };
    handler(thread_id);
}

pub(super) fn notify_reaped(reaped: Option<ThreadId>) {
    let Some(reaped) = reaped else {
        return;
    };

    let address = REAPED_HANDLER.load(Ordering::Acquire);

    if address == 0 {
        return;
    }

    // SAFETY: register_reaped_handler stores one permanent ThreadReapedHandler pointer.
    let handler: ThreadReapedHandler = unsafe { core::mem::transmute(address) };
    handler(reaped);
}

impl Scheduler {
    /// Reaps a thread only after execution has moved away from its kernel stack.
    ///
    /// Reaps an exiting thread only after its context switch has released CPU ownership. Vacant
    /// slots preserve every other CPU's `ThreadIndex` while the entry's stack and context are
    /// dropped.
    pub(super) fn reap_pending(&mut self) -> Option<ThreadId> {
        let slot = self.entries.iter_mut().find(|slot| {
            slot.as_ref().is_some_and(|entry| {
                entry.state == ThreadState::Exiting && !entry.reserved.load(Ordering::Acquire)
            })
        })?;

        Some(slot.take().expect("reap candidate disappeared").thread.id())
    }
}
