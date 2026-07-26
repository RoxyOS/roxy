use alloc::vec::Vec;
use core::{num::NonZeroU64, time::Duration};

use roxy_thread::{ThreadId, scheduler::WaitKey};
use roxy_utils::Lock;

pub(super) static TIMER_WAITERS: Lock<TimerWaiters> = Lock::new(TimerWaiters::new());

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct TimerWaiter {
    pub(super) deadline: Duration,
    pub(super) thread_id: ThreadId,
    key: WaitKey,
}

impl TimerWaiter {
    pub(super) const fn key(self) -> WaitKey {
        self.key
    }
}

pub(super) struct TimerWaiters {
    entries: Vec<TimerWaiter>,
    next_key: u64,
}

impl TimerWaiters {
    pub(super) const fn new() -> Self {
        Self {
            entries: Vec::new(),
            next_key: 1,
        }
    }

    pub(super) fn register(&mut self, thread_id: ThreadId, deadline: Duration) -> TimerWaiter {
        let key = WaitKey::new(NonZeroU64::new(self.next_key).unwrap());

        self.next_key = self
            .next_key
            .checked_add(1)
            .expect("timer wait key overflow");

        let waiter = TimerWaiter {
            deadline,
            thread_id,
            key,
        };

        self.entries.push(waiter);

        waiter
    }

    pub(super) fn take_expired(&mut self, now: Duration) -> Option<TimerWaiter> {
        let index = self
            .entries
            .iter()
            .position(|waiter| waiter.deadline <= now)?;

        Some(self.entries.swap_remove(index))
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use core::time::Duration;

    use roxy_test::kernel_test;
    use roxy_thread::Thread;

    use super::TimerWaiters;

    kernel_test!(
        "roxy-timer-wait::deadline-selection",
        removes_expired_waiters,
        {
            let thread = Thread::new(unused_thread).unwrap();
            let mut waiters = TimerWaiters::new();
            let first = waiters.register(thread.id(), Duration::from_millis(8));
            let second = waiters.register(thread.id(), Duration::from_millis(4));

            assert_eq!(waiters.take_expired(Duration::from_millis(3)), None);
            assert_eq!(waiters.take_expired(Duration::from_millis(4)), Some(second));
            assert_eq!(waiters.take_expired(Duration::from_millis(8)), Some(first));
        }
    );

    fn unused_thread() -> ! {
        panic!("unused timer-wait test thread started")
    }
}
