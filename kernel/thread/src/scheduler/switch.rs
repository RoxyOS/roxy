use alloc::boxed::Box;
use core::sync::atomic::{AtomicBool, Ordering};
use core::{cell::UnsafeCell, ptr};

use roxy_arch::{Architecture, CurrentArchitectureBackend};
use roxy_memory::activate_kernel_page_table;
use spin::Once;

use super::WaitKey;
use super::state::{BlockState, Scheduler, ThreadIndex, ThreadKind, ThreadState, local};
use crate::{SavedContext, ThreadId};

/// Activates the target user thread's address space immediately before switching to it.
type UserDispatchHook = fn(ThreadId);

static USER_DISPATCH_HOOK: Once<UserDispatchHook> = Once::new();

#[must_use = "a pending context switch must be performed"]
pub(super) struct PendingContextSwitch {
    pub(super) previous: *mut SavedContext,
    pub(super) next: *const SavedContext,
    pub(super) next_user_thread: Option<ThreadId>,
    pub(super) next_kernel_stack_top: Option<u64>,
    /// Pointer to the `reserved` flag of the thread this switch is leaving. The flag stays set
    /// while that thread still runs on (or is reserved by) its old CPU. The context-switch
    /// assembly clears it (release ordering) only after `previous` has been saved and RSP has
    /// moved onto `next`'s stack, so dispatch and reap — which read it with acquire ordering —
    /// only touch the thread once its old stack is no longer in use. `null` means switching to a
    /// CPU-local control context, which has no outgoing thread to release.
    pub(super) reserved_ptr: *const AtomicBool,
}

pub(super) struct ScheduleResult {
    pub(super) pending_switch: Option<PendingContextSwitch>,
    pub(super) reaped: Option<ThreadId>,
    pub(super) exiting: Option<ThreadId>,
}

impl PendingContextSwitch {
    pub(super) fn perform(&self) {
        prepare_dispatch(self.next_user_thread);

        if let Some(kernel_stack_top) = self.next_kernel_stack_top {
            CurrentArchitectureBackend::set_kernel_stack_top(kernel_stack_top);
        }

        // SAFETY: Stable scheduler slots own both contexts and no scheduler lock is held across
        // the switch. The backend clears `reserved_ptr` only after saving the outgoing context.
        unsafe {
            SavedContext::switch(self.previous, self.next, self.reserved_ptr);
        }
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

        assert!(local().current.is_none(), "scheduler control resumed early");
        let Some(next) = self.next_runnable(ThreadIndex(0)) else {
            return ScheduleResult {
                pending_switch: None,
                reaped,
                exiting: None,
            };
        };
        self.entry(next).state = ThreadState::Running;
        self.entry(next).reserved.store(true, Ordering::Relaxed);
        let mut local = local();
        local.current = Some(next);
        let control = local
            .control_context
            .get_or_insert_with(|| Box::new(UnsafeCell::new(SavedContext::empty())));
        let previous = control.get();
        let pending_switch = Some(self.prepare_switch_from(previous, next, ptr::null()));

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
        let Some(current) = local().current else {
            return ScheduleResult {
                pending_switch: None,
                reaped,
                exiting: None,
            };
        };

        // Record runnable intent, but retain CPU ownership until the assembly leaves this stack.
        self.entry(current).state = ThreadState::Runnable;

        let Some(next) = self.next_runnable(ThreadIndex((current.0 + 1) % self.entries.len()))
        else {
            // No runnable thread remains; keep the current thread running.
            self.entry(current).state = ThreadState::Running;
            return ScheduleResult {
                pending_switch: None,
                reaped,
                exiting: None,
            };
        };

        self.entry(next).state = ThreadState::Running;
        self.entry(next).reserved.store(true, Ordering::Relaxed);
        let previous = self.entry(current).thread.context_pointer();
        let reserved_ptr = ptr::from_ref(self.entry(current).reserved.as_ref());
        local().current = Some(next);

        ScheduleResult {
            pending_switch: Some(self.prepare_switch_from(previous, next, reserved_ptr)),
            reaped,
            exiting: None,
        }
    }

    pub(super) fn prepare_block(&mut self, wait_key: Option<WaitKey>) -> PendingContextSwitch {
        let current = local().current.expect("no current thread");

        self.entry(current).state = ThreadState::Blocked(match wait_key {
            Some(wait_key) => BlockState::Keyed(wait_key),
            None => BlockState::Unkeyed,
        });

        self.prepare_block_switch(current)
    }

    /// Clears `latch` and blocks with the caller's wait key unless the latch was set.
    ///
    /// The notifier sets the latch before requesting a keyed wake. An early notification therefore
    /// skips the block and leaves the thread running so the caller can recheck readiness.
    pub(super) fn prepare_block_with_latch(
        &mut self,
        wait_key: WaitKey,
        latch: &AtomicBool,
    ) -> Option<PendingContextSwitch> {
        let current = local().current.expect("no current thread");

        if latch.swap(false, Ordering::SeqCst) {
            return None;
        }

        self.entry(current).state = ThreadState::Blocked(BlockState::Keyed(wait_key));
        Some(self.prepare_block_switch(current))
    }

    /// Builds the outgoing switch from the caller-set block state of `current` to the next
    /// runnable thread (or the scheduler control context when none remains).
    fn prepare_block_switch(&mut self, current: ThreadIndex) -> PendingContextSwitch {
        let previous = self.entry(current).thread.context_pointer();
        let reserved_ptr = ptr::from_ref(self.entry(current).reserved.as_ref());
        let next = self.next_runnable(ThreadIndex((current.0 + 1) % self.entries.len()));

        let mut local = local();
        local.current = next;

        let Some(next) = next else {
            return PendingContextSwitch {
                previous,
                next: local
                    .control_context
                    .as_ref()
                    .expect("scheduler control context missing")
                    .get(),
                next_user_thread: None,
                next_kernel_stack_top: None,
                reserved_ptr,
            };
        };

        self.entry(next).state = ThreadState::Running;
        self.entry(next).reserved.store(true, Ordering::Relaxed);
        self.prepare_switch_from(previous, next, reserved_ptr)
    }

    pub(super) fn wake_unconditionally(&mut self, thread_id: ThreadId) -> bool {
        let Some(index) = self.index_of(thread_id) else {
            return false;
        };

        if !matches!(self.entry(index).state, ThreadState::Blocked(_)) {
            return false;
        }

        self.entry(index).state = ThreadState::Runnable;

        true
    }

    pub(super) fn wake_if_waiting(&mut self, thread_id: ThreadId, wait_key: WaitKey) -> bool {
        let Some(index) = self.index_of(thread_id) else {
            return false;
        };

        if self.entry(index).state != ThreadState::Blocked(BlockState::Keyed(wait_key)) {
            return false;
        }

        self.entry(index).state = ThreadState::Runnable;

        true
    }

    /// Marks the current thread for deferred reaping and prepares its final switch away.
    ///
    /// The target is the next runnable thread, or the scheduler control context when none remain.
    /// The current thread stays owned by the scheduler until execution is using another stack.
    /// The returned switch is performed after releasing the scheduler lock.
    pub(super) fn prepare_exit(&mut self) -> ScheduleResult {
        let reaped = self.reap_pending();
        let current = local().current.expect("no current thread");
        self.entry(current).state = ThreadState::Exiting;
        let next = self.next_runnable(ThreadIndex((current.0 + 1) % self.entries.len()));
        local().current = next;

        let previous = self.entry(current).thread.context_pointer();
        let reserved_ptr = ptr::from_ref(self.entry(current).reserved.as_ref());
        if let Some(next) = next {
            self.entry(next).state = ThreadState::Running;
            self.entry(next).reserved.store(true, Ordering::Relaxed);
        }
        let pending_switch = match next {
            Some(next) => self.prepare_switch_from(previous, next, reserved_ptr),
            None => PendingContextSwitch {
                previous,
                next: local()
                    .control_context
                    .as_ref()
                    .expect("scheduler not started")
                    .get(),
                next_user_thread: None,
                next_kernel_stack_top: None,
                reserved_ptr,
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
        reserved_ptr: *const AtomicBool,
    ) -> PendingContextSwitch {
        let entry = self.entry(next);
        let next_user_thread = match entry.kind {
            ThreadKind::Kernel => None,
            ThreadKind::User => Some(entry.thread.id()),
        };
        let next_kernel_stack_top = entry.thread.kernel_stack_top().as_u64();
        let next_context = entry.thread.context_pointer();

        PendingContextSwitch {
            previous,
            next: next_context,
            next_user_thread,
            next_kernel_stack_top: Some(next_kernel_stack_top),
            reserved_ptr,
        }
    }

    fn next_runnable(&self, start: ThreadIndex) -> Option<ThreadIndex> {
        (0..self.entries.len()).find_map(|offset| {
            let index = (start.0 + offset) % self.entries.len();
            let entry = self.entries[index].as_ref()?;
            (entry.state == ThreadState::Runnable && !entry.reserved.load(Ordering::Acquire))
                .then_some(ThreadIndex(index))
        })
    }
}

pub(super) fn register_user_dispatch_hook(hook: UserDispatchHook) {
    assert!(
        USER_DISPATCH_HOOK.get().is_none(),
        "user dispatch hook registered twice"
    );
    USER_DISPATCH_HOOK.call_once(|| hook);
}

fn prepare_dispatch(user_thread: Option<ThreadId>) {
    let Some(thread_id) = user_thread else {
        activate_kernel_page_table();
        return;
    };
    let hook = USER_DISPATCH_HOOK
        .get()
        .expect("user dispatch hook is not registered");

    hook(thread_id);
}

#[cfg(feature = "kernel-test")]
mod tests {
    use core::num::NonZeroU64;
    use core::sync::atomic::{AtomicBool, Ordering};

    use roxy_test::kernel_test;

    use super::ThreadKind;
    use super::local;
    use super::{BlockState, Scheduler, ThreadIndex, ThreadState};
    use crate::{Thread, scheduler::WaitKey};

    kernel_test!("roxy-thread::scheduler-block-wake", scheduler_block_wake, {
        let first = Thread::new(unused_thread).unwrap();
        let first_id = first.id();
        let second = Thread::new(unused_thread).unwrap();
        let mut scheduler = Scheduler::new();
        scheduler.enqueue(first, ThreadKind::Kernel);
        scheduler.enqueue(second, ThreadKind::Kernel);
        let saved_current = local().current;
        local().current = Some(ThreadIndex(0));
        scheduler.entry(ThreadIndex(0)).state = ThreadState::Running;
        scheduler
            .entry(ThreadIndex(0))
            .reserved
            .store(true, Ordering::Relaxed);

        let pending = scheduler.prepare_block(None);
        assert_eq!(
            scheduler.entry(ThreadIndex(0)).state,
            ThreadState::Blocked(BlockState::Unkeyed)
        );
        assert_eq!(local().current, Some(ThreadIndex(1)));
        assert!(scheduler.wake_unconditionally(first_id));
        assert_eq!(scheduler.entry(ThreadIndex(0)).state, ThreadState::Runnable);
        assert!(!scheduler.wake_unconditionally(first_id));
        assert_eq!(scheduler.next_runnable(ThreadIndex(0)), None);
        // Simulate the assembly handoff only after checking that an early wake cannot dispatch.
        assert_eq!(
            pending.reserved_ptr,
            core::ptr::from_ref(scheduler.entry(ThreadIndex(0)).reserved.as_ref())
        );
        scheduler
            .entry(ThreadIndex(0))
            .reserved
            .store(false, Ordering::Release);
        assert_eq!(
            scheduler.next_runnable(ThreadIndex(0)),
            Some(ThreadIndex(0))
        );
        local().current = saved_current;
    });

    kernel_test!(
        "roxy-thread::scheduler-block-with-latch",
        scheduler_block_with_latch,
        {
            let first = Thread::new(unused_thread).unwrap();
            let second = Thread::new(unused_thread).unwrap();
            let mut scheduler = Scheduler::new();
            scheduler.enqueue(first, ThreadKind::Kernel);
            scheduler.enqueue(second, ThreadKind::Kernel);

            let saved_current = local().current;
            scheduler
                .entry(ThreadIndex(0))
                .reserved
                .store(true, Ordering::Relaxed);
            // A set latch (a wake owed to a still-running thread) must not block or switch away;
            // the thread keeps running and the caller re-checks its readiness.
            local().current = Some(ThreadIndex(0));
            scheduler.entry(ThreadIndex(0)).state = ThreadState::Running;
            let latched = AtomicBool::new(true);
            let key = WaitKey::new(NonZeroU64::new(7).unwrap());
            let pending = scheduler.prepare_block_with_latch(key, &latched);
            assert!(pending.is_none());
            assert_eq!(scheduler.entry(ThreadIndex(0)).state, ThreadState::Running);
            assert!(!latched.load(Ordering::SeqCst));
            assert_eq!(local().current, Some(ThreadIndex(0)));
            assert!(
                scheduler
                    .entry(ThreadIndex(0))
                    .reserved
                    .load(Ordering::Acquire)
            );

            // A cleared latch blocks normally and prepares a switch away.
            local().current = Some(ThreadIndex(0));
            scheduler.entry(ThreadIndex(0)).state = ThreadState::Running;
            let clear = AtomicBool::new(false);
            let other_key = WaitKey::new(NonZeroU64::new(8).unwrap());
            let pending2 = scheduler.prepare_block_with_latch(other_key, &clear);
            assert!(pending2.is_some());
            assert_eq!(
                scheduler.entry(ThreadIndex(0)).state,
                ThreadState::Blocked(BlockState::Keyed(other_key))
            );
            let first_id = scheduler.entry(ThreadIndex(0)).thread.id();
            assert!(scheduler.wake_if_waiting(first_id, other_key));
            assert_eq!(scheduler.next_runnable(ThreadIndex(0)), None);
            scheduler
                .entry(ThreadIndex(0))
                .reserved
                .store(false, Ordering::Release);
            assert_eq!(
                scheduler.next_runnable(ThreadIndex(0)),
                Some(ThreadIndex(0))
            );
            local().current = saved_current;
        }
    );

    kernel_test!(
        "roxy-thread::preemption-retains-cpu-ownership",
        preemption_ownership,
        {
            let saved_current = local().current;
            let mut scheduler = Scheduler::new();
            scheduler.enqueue(Thread::new(unused_thread).unwrap(), ThreadKind::Kernel);
            scheduler.enqueue(Thread::new(unused_thread).unwrap(), ThreadKind::Kernel);
            local().current = Some(ThreadIndex(0));
            scheduler.entry(ThreadIndex(0)).state = ThreadState::Running;
            scheduler
                .entry(ThreadIndex(0))
                .reserved
                .store(true, Ordering::Relaxed);
            let prepared = scheduler.prepare_preemption();
            assert!(prepared.pending_switch.is_some());
            assert_eq!(scheduler.entry(ThreadIndex(0)).state, ThreadState::Runnable);
            assert_eq!(scheduler.next_runnable(ThreadIndex(0)), None);
            scheduler
                .entry(ThreadIndex(0))
                .reserved
                .store(false, Ordering::Release);
            assert_eq!(
                scheduler.next_runnable(ThreadIndex(0)),
                Some(ThreadIndex(0))
            );
            local().current = saved_current;
        }
    );

    kernel_test!(
        "roxy-thread::reap-waits-for-stack-handoff",
        reap_ownership,
        {
            let saved_current = local().current;
            let mut scheduler = Scheduler::new();
            scheduler.enqueue(Thread::new(unused_thread).unwrap(), ThreadKind::Kernel);
            scheduler.enqueue(Thread::new(unused_thread).unwrap(), ThreadKind::Kernel);
            let first_id = scheduler.entry(ThreadIndex(0)).thread.id();
            let second_id = scheduler.entry(ThreadIndex(1)).thread.id();
            local().current = Some(ThreadIndex(0));
            scheduler.entry(ThreadIndex(0)).state = ThreadState::Running;
            scheduler
                .entry(ThreadIndex(0))
                .reserved
                .store(true, Ordering::Relaxed);
            let prepared = scheduler.prepare_exit();
            assert_eq!(prepared.exiting, Some(first_id));
            assert_eq!(scheduler.reap_pending(), None);
            scheduler
                .entry(ThreadIndex(0))
                .reserved
                .store(false, Ordering::Release);
            assert_eq!(scheduler.reap_pending(), Some(first_id));
            assert_eq!(scheduler.reap_pending(), None);
            assert_eq!(local().current, Some(ThreadIndex(1)));
            assert_eq!(scheduler.current_thread_id(), second_id);
            scheduler.enqueue(Thread::new(unused_thread).unwrap(), ThreadKind::Kernel);
            assert_eq!(scheduler.entries.len(), 2);
            assert_eq!(scheduler.current_thread_id(), second_id);
            assert_eq!(scheduler.index_of(first_id), None);
            local().current = saved_current;
        }
    );

    kernel_test!(
        "roxy-thread::context-survives-queue-growth",
        stable_context,
        {
            let mut scheduler = Scheduler::new();
            scheduler.enqueue(Thread::new(unused_thread).unwrap(), ThreadKind::Kernel);
            let context = scheduler.entry(ThreadIndex(0)).thread.context_pointer();
            let ownership = core::ptr::from_ref(scheduler.entry(ThreadIndex(0)).reserved.as_ref());
            let entry = core::ptr::from_ref(scheduler.entry(ThreadIndex(0)));
            let capacity = scheduler.entries.capacity();
            while scheduler.entries.len() <= capacity {
                scheduler.enqueue(Thread::new(unused_thread).unwrap(), ThreadKind::Kernel);
            }
            assert!(scheduler.entries.capacity() > capacity);
            assert_eq!(
                scheduler.entry(ThreadIndex(0)).thread.context_pointer(),
                context
            );
            assert_eq!(
                core::ptr::from_ref(scheduler.entry(ThreadIndex(0)).reserved.as_ref()),
                ownership
            );
            assert_eq!(core::ptr::from_ref(scheduler.entry(ThreadIndex(0))), entry);
        }
    );

    fn unused_thread() -> ! {
        panic!("unused scheduler test thread started")
    }
}
