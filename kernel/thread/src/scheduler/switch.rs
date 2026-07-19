use core::ptr;

use roxy_arch::{Architecture, CurrentArchitectureBackend};

use super::{
    addrspace::ScheduledAddrSpace,
    state::{Scheduler, ThreadIndex, ThreadState},
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
    pub(super) exiting: Option<ThreadId>,
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
                exiting: None,
            };
        }

        assert!(self.current.is_none(), "scheduler control resumed early");
        let Some(next) = self.next_runnable(ThreadIndex(0)) else {
            return ScheduleResult {
                pending_switch: None,
                reaped,
                exiting: None,
            };
        };
        let control = self.control_context.get_or_insert_with(SavedContext::empty);
        self.current = Some(next);
        let previous = ptr::from_mut(control);
        let pending_switch = Some(self.prepare_switch_from(previous, next));

        ScheduleResult {
            pending_switch,
            reaped,
            exiting: None,
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
                exiting: None,
            };
        };

        let Some(next) = self.next_runnable(ThreadIndex((current.0 + 1) % self.entries.len()))
        else {
            return ScheduleResult {
                pending_switch: None,
                reaped,
                exiting: None,
            };
        };
        if next == current {
            return ScheduleResult {
                pending_switch: None,
                reaped,
                exiting: None,
            };
        }

        let previous = ptr::from_mut(self.entry(current).thread.context());
        self.current = Some(next);

        ScheduleResult {
            pending_switch: Some(self.prepare_switch_from(previous, next)),
            reaped,
            exiting: None,
        }
    }

    pub(super) fn prepare_block(&mut self) -> PendingContextSwitch {
        let current = self.current.expect("no current thread");
        self.entry(current).state = ThreadState::Blocked;
        let previous = ptr::from_mut(self.entry(current).thread.context());
        let next = self.next_runnable(ThreadIndex((current.0 + 1) % self.entries.len()));
        self.current = next;

        match next {
            Some(next) => self.prepare_switch_from(previous, next),
            None => PendingContextSwitch {
                previous,
                next: ptr::from_ref(self.control_context.get_or_insert_with(SavedContext::empty)),
                next_addrspace: ScheduledAddrSpace::Kernel,
                next_kernel_stack_top: None,
            },
        }
    }

    pub(super) fn wake(&mut self, thread_id: ThreadId) -> bool {
        let Some(index) = self.index_of(thread_id) else {
            return false;
        };
        let entry = self.entry(index);
        if entry.state != ThreadState::Blocked {
            return false;
        }
        entry.state = ThreadState::Runnable;
        true
    }

    /// Marks the current thread for deferred reaping and prepares its final switch away.
    ///
    /// The target is the next runnable thread, or the scheduler control context when none remain.
    /// The current thread stays owned by the scheduler until execution is using another stack.
    /// The returned switch is performed after releasing the scheduler lock.
    pub(super) fn prepare_exit(&mut self) -> ScheduleResult {
        let reaped = self.reap_pending();
        let current = self.current.expect("no current thread");
        self.entry(current).state = ThreadState::Exiting;
        let next = self.next_runnable(ThreadIndex((current.0 + 1) % self.entries.len()));
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
            exiting: Some(self.entry(current).thread.id()),
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

    fn next_runnable(&self, start: ThreadIndex) -> Option<ThreadIndex> {
        (0..self.entries.len()).find_map(|offset| {
            let index = (start.0 + offset) % self.entries.len();
            (self.entries[index].state == ThreadState::Runnable).then_some(ThreadIndex(index))
        })
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_test::kernel_test;

    use super::{ScheduledAddrSpace, Scheduler, ThreadIndex, ThreadState};
    use crate::Thread;

    kernel_test!("roxy-thread::scheduler-block-wake", scheduler_block_wake, {
        let first = Thread::new(unused_thread).unwrap();
        let first_id = first.id();
        let second = Thread::new(unused_thread).unwrap();
        let mut scheduler = Scheduler::new();
        scheduler.enqueue(first, ScheduledAddrSpace::Kernel);
        scheduler.enqueue(second, ScheduledAddrSpace::Kernel);
        scheduler.current = Some(ThreadIndex(0));

        let _pending = scheduler.prepare_block();
        assert_eq!(scheduler.entries[0].state, ThreadState::Blocked);
        assert_eq!(scheduler.current, Some(ThreadIndex(1)));
        assert!(scheduler.wake(first_id));
        assert_eq!(scheduler.entries[0].state, ThreadState::Runnable);
        assert!(!scheduler.wake(first_id));
    });

    fn unused_thread() -> ! {
        panic!("unused scheduler test thread started")
    }
}
