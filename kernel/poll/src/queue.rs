use alloc::{sync::Arc, vec::Vec};

use roxy_utils::Lock;

use crate::PollListener;

/// A readiness source's collection of blocked poll listeners.
pub struct PollListeners {
    entries: Lock<PollWaitEntries>,
}

struct PollWaitEntries {
    next_registration: u64,
    listeners: Vec<RegisteredListener>,
}

struct RegisteredListener {
    id: u64,
    listener: Arc<PollListener>,
}

impl PollListeners {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Lock::new(PollWaitEntries {
                next_registration: 1,
                listeners: Vec::new(),
            }),
        }
    }

    #[must_use]
    pub fn register(self: &Arc<Self>, listener: Arc<PollListener>) -> PollRegistration {
        let mut entries = self.entries.lock();
        let id = entries.next_registration;
        entries.next_registration = entries
            .next_registration
            .checked_add(1)
            .expect("poll registration ID overflow");
        entries.listeners.push(RegisteredListener { id, listener });

        PollRegistration::Active {
            queue: self.clone(),
            id,
        }
    }

    pub fn notify(&self) {
        let entries = self.entries.lock();

        for entry in &entries.listeners {
            entry.listener.wake();
        }
    }

    fn unregister(&self, id: u64) {
        let mut entries = self.entries.lock();
        let index = entries
            .listeners
            .iter()
            .position(|entry| entry.id == id)
            .expect("poll registration must remain queued until its guard drops");
        entries.listeners.swap_remove(index);
    }

    #[cfg(feature = "kernel-test")]
    fn count(&self) -> usize {
        self.entries.lock().listeners.len()
    }
}

impl Default for PollListeners {
    fn default() -> Self {
        Self::new()
    }
}

/// Keeps one readiness listener registered until dropped.
pub enum PollRegistration {
    Active { queue: Arc<PollListeners>, id: u64 },
    Inactive,
}

impl PollRegistration {
    #[must_use]
    pub const fn inactive() -> Self {
        Self::Inactive
    }
}

impl Drop for PollRegistration {
    fn drop(&mut self) {
        if let Self::Active { queue, id } = self {
            queue.unregister(*id);
        }
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use alloc::sync::Arc;
    use core::num::NonZeroU64;

    use roxy_test::kernel_test;
    use roxy_thread::{Thread, scheduler::WaitKey};

    use super::PollListeners;
    use crate::PollListener;

    kernel_test!(
        "roxy-poll::registration-drop",
        removes_only_its_registration,
        {
            let queue = Arc::new(PollListeners::new());
            let first = queue.register(listener());
            let second = queue.register(listener());

            assert_eq!(queue.count(), 2);
            drop(first);
            assert_eq!(queue.count(), 1);
            drop(second);
            assert_eq!(queue.count(), 0);
        }
    );

    fn listener() -> Arc<PollListener> {
        let thread = Thread::new(unused_thread).unwrap();
        let wait_key = WaitKey::new(NonZeroU64::new(1).unwrap());

        Arc::new(PollListener::for_test(thread.id(), wait_key))
    }

    fn unused_thread() -> ! {
        panic!("unused poll listener thread started")
    }
}
