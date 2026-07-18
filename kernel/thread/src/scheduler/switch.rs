use core::ptr;

use roxy_arch::{Architecture, CurrentArchitectureBackend};

use super::{
    addrspace::ScheduledAddrSpace,
    state::{Scheduler, ThreadIndex},
};
use crate::{SavedContext, ThreadId};

#[must_use = "a pending context switch must be performed"]
pub(super) struct PendingContextSwitch {
    pub(super) previous: *mut SavedContext,
    pub(super) next: *const SavedContext,
    pub(super) next_addrspace: ScheduledAddrSpace,
    pub(super) next_kernel_stack_top: Option<u64>,
}

pub(super) struct ScheduleResult {
    pub(super) pending_switch: Option<PendingContextSwitch>,
    pub(super) reaped: Option<ThreadId>,
}

impl PendingContextSwitch {
    pub(super) fn perform(&self) {
        self.next_addrspace.activate_if_needed();

        if let Some(kernel_stack_top) = self.next_kernel_stack_top {
            CurrentArchitectureBackend::set_kernel_stack_top(kernel_stack_top);
        }

        // SAFETY: Scheduler entries own both contexts and no scheduler lock is held across the
        // switch.
        unsafe { SavedContext::switch(self.previous, self.next) };
    }
}

impl Scheduler {
    /// Prepares a switch from the scheduler control context to the first runnable thread.
    ///
    /// This is used when the scheduler starts or regains control after the last thread exits. It
    /// also completes any deferred reap. The returned switch is performed after releasing the
    /// scheduler lock; no context switch occurs in this method.
    pub(super) fn prepare_dispatch(&mut self) -> ScheduleResult {
        let reaped = self.reap_pending();
        if self.entries.is_empty() {
            return ScheduleResult {
                pending_switch: None,
                reaped,
            };
        }

        assert!(self.current.is_none(), "scheduler control resumed early");
        let control = self.control_context.get_or_insert_with(SavedContext::empty);
        self.current = Some(ThreadIndex(0));
        let previous = ptr::from_mut(control);
        let pending_switch = Some(self.prepare_switch_from(previous, ThreadIndex(0)));

        ScheduleResult {
            pending_switch,
            reaped,
        }
    }

    /// Prepares a timer-driven round-robin switch to the next runnable thread.
    ///
    /// A switch is omitted when no thread is active or only one thread is runnable. Any deferred
    /// reap is still completed. The returned switch is performed after releasing the scheduler
    /// lock; no context switch occurs in this method.
    pub(super) fn prepare_preemption(&mut self) -> ScheduleResult {
        let reaped = self.reap_pending();
        let Some(current) = self.current else {
            return ScheduleResult {
                pending_switch: None,
                reaped,
            };
        };

        if self.entries.len() < 2 {
            return ScheduleResult {
                pending_switch: None,
                reaped,
            };
        }

        let next = ThreadIndex((current.0 + 1) % self.entries.len());
        let previous = ptr::from_mut(self.entry(current).thread.context());
        self.current = Some(next);

        ScheduleResult {
            pending_switch: Some(self.prepare_switch_from(previous, next)),
            reaped,
        }
    }

    /// Marks the current thread for deferred reaping and prepares its final switch away.
    ///
    /// The target is the next runnable thread, or the scheduler control context when none remain.
    /// The current thread stays owned by the scheduler until execution is using another stack.
    /// The returned switch is performed after releasing the scheduler lock.
    pub(super) fn prepare_exit(&mut self) -> ScheduleResult {
        let reaped = self.reap_pending();
        let current = self.current.expect("no current thread");
        let next =
            (self.entries.len() > 1).then(|| ThreadIndex((current.0 + 1) % self.entries.len()));
        self.current = next;
        self.pending_reap = Some(current);

        let previous = ptr::from_mut(self.entry(current).thread.context());
        let pending_switch = match next {
            Some(next) => self.prepare_switch_from(previous, next),
            None => PendingContextSwitch {
                previous,
                next: ptr::from_ref(
                    self.control_context
                        .as_ref()
                        .expect("scheduler not started"),
                ),
                next_addrspace: ScheduledAddrSpace::Kernel,
                next_kernel_stack_top: None,
            },
        };

        ScheduleResult {
            pending_switch: Some(pending_switch),
            reaped,
        }
    }

    fn prepare_switch_from(
        &mut self,
        previous: *mut SavedContext,
        next: ThreadIndex,
    ) -> PendingContextSwitch {
        let entry = self.entry(next);
        PendingContextSwitch {
            previous,
            next: ptr::from_mut(entry.thread.context()),
            next_addrspace: entry.addrspace.clone(),
            next_kernel_stack_top: Some(entry.thread.kernel_stack_top().as_u64()),
        }
    }
}
