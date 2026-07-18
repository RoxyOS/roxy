use core::sync::atomic::{AtomicUsize, Ordering};

use super::state::Scheduler;
use crate::ThreadId;

static REAPED_HANDLER: AtomicUsize = AtomicUsize::new(0);

pub type ThreadReapedHandler = fn(ThreadId);

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
    /// `exit_current` cannot remove the active entry because `rsp` still points into its kernel
    /// stack. It records the index in `pending_reap` and switches away. The next scheduler entry
    /// removes it from a different stack, drops its address-space handle, and repairs `current` if
    /// `Vec::remove` shifted the successor's index.
    pub(super) fn reap_pending(&mut self) -> Option<ThreadId> {
        let pending_reap = self.pending_reap.take()?;
        assert!(
            self.current != Some(pending_reap),
            "cannot reap active thread"
        );
        let reaped = self.entries.remove(pending_reap.0).thread.id();

        if let Some(current) = &mut self.current
            && current.0 > pending_reap.0
        {
            current.0 -= 1;
        }

        Some(reaped)
    }
}
