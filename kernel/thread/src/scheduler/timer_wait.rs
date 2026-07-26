use core::time::Duration;

use super::state::{Scheduler, ThreadState};
use crate::ThreadId;

/// Identifies one timer-wait registration, rather than the thread that owns it.
///
/// A thread can block repeatedly over its lifetime. Matching this token against the scheduler
/// entry prevents an expired registration from an earlier wait from waking a later wait by the
/// same thread.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct WaitToken(u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct TimerWaiter {
    deadline: Duration,
    thread_id: ThreadId,
    token: WaitToken,
}

impl Scheduler {
    pub(super) fn register_timer_waiter(
        &mut self,
        thread_id: ThreadId,
        deadline: Duration,
    ) -> WaitToken {
        let token = WaitToken(self.next_wait_token);
        self.next_wait_token = self
            .next_wait_token
            .checked_add(1)
            .expect("wait token overflow");
        self.timer_waiters.push(TimerWaiter {
            deadline,
            thread_id,
            token,
        });

        token
    }

    pub(super) fn remove_timer_waiter(&mut self, token: WaitToken) {
        self.timer_waiters.retain(|waiter| waiter.token != token);
    }

    pub(super) fn wake_expired(&mut self, now: Duration) {
        let mut index = 0;

        while index < self.timer_waiters.len() {
            if self.timer_waiters[index].deadline > now {
                index += 1;
                continue;
            }

            let waiter = self.timer_waiters.swap_remove(index);
            let Some(thread_index) = self.index_of(waiter.thread_id) else {
                continue;
            };

            let entry = self.entry(thread_index);

            if entry.state == ThreadState::Blocked && entry.current_timer_wait == Some(waiter.token)
            {
                entry.current_timer_wait = None;
                entry.state = ThreadState::Runnable;
            }
        }
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use core::time::Duration;

    use roxy_test::kernel_test;

    use super::Scheduler;
    use crate::{
        Thread,
        scheduler::state::{ThreadIndex, ThreadKind, ThreadState},
    };

    kernel_test!(
        "roxy-thread::timer-waiter-deadline",
        deadline_wakes_thread,
        {
            let thread = Thread::new(unused_thread).unwrap();
            let mut scheduler = Scheduler::new();
            scheduler.enqueue(thread, ThreadKind::Kernel);
            scheduler.current = Some(ThreadIndex(0));

            let _pending = scheduler.prepare_block(Some(Duration::from_millis(8)));
            scheduler.wake_expired(Duration::from_millis(7));
            assert_eq!(scheduler.entries[0].state, ThreadState::Blocked);
            scheduler.wake_expired(Duration::from_millis(8));
            assert_eq!(scheduler.entries[0].state, ThreadState::Runnable);
            assert!(scheduler.timer_waiters.is_empty());
        }
    );

    kernel_test!("roxy-thread::timer-waiter-cancel", explicit_wake_cancels, {
        let thread = Thread::new(unused_thread).unwrap();
        let thread_id = thread.id();
        let mut scheduler = Scheduler::new();
        scheduler.enqueue(thread, ThreadKind::Kernel);
        scheduler.current = Some(ThreadIndex(0));

        let _pending = scheduler.prepare_block(Some(Duration::from_millis(8)));
        assert!(scheduler.wake(thread_id));
        assert!(scheduler.timer_waiters.is_empty());
        scheduler.wake_expired(Duration::from_millis(8));
        assert_eq!(scheduler.entries[0].state, ThreadState::Runnable);
    });

    fn unused_thread() -> ! {
        panic!("unused scheduler test thread started")
    }
}
