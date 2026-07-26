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

    pub(super) fn next_key(&mut self) -> WaitKey {
        let key = WaitKey::new(NonZeroU64::new(self.next_key).unwrap());

        self.next_key = self
            .next_key
            .checked_add(1)
            .expect("timer wait key overflow");

        key
    }

    pub(super) fn register(&mut self, thread_id: ThreadId, deadline: Duration, key: WaitKey) {
        let waiter = TimerWaiter {
            deadline,
            thread_id,
            key,
        };

        self.entries.push(waiter);
    }

    pub(super) fn take_expired(&mut self, now: Duration) -> Option<TimerWaiter> {
        let index = self
            .entries
            .iter()
            .position(|waiter| waiter.deadline <= now)?;

        Some(self.entries.swap_remove(index))
    }

    pub(super) fn cancel(&mut self, key: WaitKey) {
        let Some(index) = self.entries.iter().position(|waiter| waiter.key == key) else {
            return;
        };

        self.entries.swap_remove(index);
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
            let first_key = waiters.next_key();
            let second_key = waiters.next_key();
            waiters.register(thread.id(), Duration::from_millis(8), first_key);
            waiters.register(thread.id(), Duration::from_millis(4), second_key);

            assert_eq!(waiters.take_expired(Duration::from_millis(3)), None);
            assert_eq!(
                waiters
                    .take_expired(Duration::from_millis(4))
                    .unwrap()
                    .key(),
                second_key
            );
            assert_eq!(
                waiters
                    .take_expired(Duration::from_millis(8))
                    .unwrap()
                    .key(),
                first_key
            );
        }
    );

    kernel_test!("roxy-timer-wait::deadline-cancellation", removes_waiter, {
        let thread = Thread::new(unused_thread).unwrap();
        let mut waiters = TimerWaiters::new();
        let key = waiters.next_key();
        waiters.register(thread.id(), Duration::from_millis(4), key);

        waiters.cancel(key);

        assert_eq!(waiters.take_expired(Duration::from_millis(4)), None);
    });

    fn unused_thread() -> ! {
        panic!("unused timer-wait test thread started")
    }
}
