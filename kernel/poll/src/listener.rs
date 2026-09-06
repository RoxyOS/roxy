use alloc::sync::Arc;
use core::{
    num::NonZeroU64,
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
};

use roxy_thread::{ThreadId, scheduler};

static NEXT_WAIT_KEY: AtomicU64 = AtomicU64::new(1);

/// Identifies one blocked poll operation and wakes it when a source changes state.
///
/// `notified` records whether a source already signalled this listener since the last block,
/// regardless of whether the scheduler accepted the wake. It turns the SMP lost-wakeup window
/// (a notification arriving while the owner thread is still `Running`) into a non-blocking wake:
/// [`scheduler::prepare_block_current_with_key_and_latch`] consumes it and leaves the thread
/// runnable instead of sleeping.
pub struct PollListener {
    thread_id: ThreadId,
    wait_key: scheduler::WaitKey,
    notified: AtomicBool,
}

impl PollListener {
    #[must_use]
    pub fn current_thread() -> Arc<Self> {
        let value = NEXT_WAIT_KEY.fetch_add(1, Ordering::Relaxed);
        let wait_key =
            scheduler::WaitKey::new(NonZeroU64::new(value).expect("poll wait key overflow"));

        Arc::new(Self {
            thread_id: scheduler::current_thread_id(),
            wait_key,
            notified: AtomicBool::new(false),
        })
    }

    #[must_use]
    pub const fn wait_key(&self) -> scheduler::WaitKey {
        self.wait_key
    }

    /// Borrows the wake latch this listener hands to the scheduler block.
    #[must_use]
    pub fn notified(&self) -> &AtomicBool {
        &self.notified
    }

    pub fn wake(&self) {
        // Record the owed wake before asking the scheduler, so the latch survives a wake that
        // arrives before the owner thread is blocked (the scheduler drops such a wake today).
        self.notified.store(true, Ordering::SeqCst);
        let _ = scheduler::wake_if_waiting(self.thread_id, self.wait_key);
    }

    #[cfg(feature = "kernel-test")]
    pub(crate) const fn for_test(thread_id: ThreadId, wait_key: scheduler::WaitKey) -> Self {
        Self {
            thread_id,
            wait_key,
            notified: AtomicBool::new(false),
        }
    }
}
