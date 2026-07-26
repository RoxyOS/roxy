use alloc::sync::Arc;
use core::{
    num::NonZeroU64,
    sync::atomic::{AtomicU64, Ordering},
};

use roxy_thread::{ThreadId, scheduler};

static NEXT_WAIT_KEY: AtomicU64 = AtomicU64::new(1);

/// Identifies one blocked poll operation and wakes it when a source changes state.
pub struct PollListener {
    thread_id: ThreadId,
    wait_key: scheduler::WaitKey,
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
        })
    }

    #[must_use]
    pub const fn wait_key(&self) -> scheduler::WaitKey {
        self.wait_key
    }

    pub fn wake(&self) {
        let _ = scheduler::wake_if_waiting(self.thread_id, self.wait_key);
    }

    #[cfg(feature = "kernel-test")]
    pub(crate) const fn for_test(thread_id: ThreadId, wait_key: scheduler::WaitKey) -> Self {
        Self {
            thread_id,
            wait_key,
        }
    }
}
