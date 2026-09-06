use alloc::vec::Vec;

use roxy_cpu::CpuLocal;
use roxy_utils::Lock;

use super::WaitKey;
use crate::{SavedContext, Thread, ThreadId};

/// Per-CPU slice of scheduler state: which thread this CPU is running and its scheduler
/// control-context save area.
///
/// This is the part of the scheduler that belongs to one CPU rather than being shared across all
/// of them. `current` names the thread running on this CPU; `control_context` is where this CPU's
/// scheduler control loop saves its own resume state while a thread runs. Each CPU owns its own
/// slot in the `LOCAL` per-CPU storage below.
pub(super) struct LocalScheduler {
    pub(super) current: Option<ThreadIndex>,
    pub(super) control_context: Option<SavedContext>,
}

impl LocalScheduler {
    pub(super) const fn new() -> Self {
        Self {
            current: None,
            control_context: None,
        }
    }
}

/// Per-CPU scheduler storage: a `Lock<LocalScheduler>` per active CPU, accessed through the
/// established `CpuLocal<Lock<T>>` idiom (see `LOCAL_APIC`, `APIC_TIMER`, `IO_APIC`).
///
/// Each CPU initialises its own slot once before entering the scheduler control loop (BSP during
/// `scheduler::initialize`, each AP in its own `ap_main_2`). After that `LOCAL.get().lock()`
/// gives exclusive mutable access to the current CPU's `LocalScheduler`.
static LOCAL: CpuLocal<Lock<LocalScheduler>> = CpuLocal::new();

pub(super) struct Scheduler {
    pub(super) entries: Vec<SchedulerEntry>,
    pub(super) pending_reap: Option<ThreadIndex>,
}

pub(super) struct SchedulerEntry {
    pub(super) thread: Thread,
    pub(super) kind: ThreadKind,
    pub(super) state: ThreadState,
}

#[derive(Clone, Copy)]
pub(super) enum ThreadKind {
    Kernel,
    User,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ThreadState {
    Runnable,
    /// Currently running on one CPU; not available for dispatch by other CPUs.
    Running,
    Blocked(BlockState),
    Exiting,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum BlockState {
    Unkeyed,
    Keyed(WaitKey),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ThreadIndex(pub(super) usize);

impl Scheduler {
    pub(super) const fn new() -> Self {
        Self {
            entries: Vec::new(),
            pending_reap: None,
        }
    }

    pub(super) fn enqueue(&mut self, thread: Thread, kind: ThreadKind) {
        self.entries.push(SchedulerEntry {
            thread,
            kind,
            state: ThreadState::Runnable,
        });
    }

    pub(super) fn current_thread_id(&mut self) -> ThreadId {
        let current = local().current.expect("no current thread");
        self.entry(current).thread.id()
    }

    pub(super) fn try_current_thread_id(&self) -> Option<ThreadId> {
        local()
            .current
            .map(|current| self.entries[current.0].thread.id())
    }

    pub(super) fn entry(&mut self, index: ThreadIndex) -> &mut SchedulerEntry {
        &mut self.entries[index.0]
    }

    pub(super) fn index_of(&self, thread_id: ThreadId) -> Option<ThreadIndex> {
        self.entries
            .iter()
            .position(|entry| entry.thread.id() == thread_id)
            .map(ThreadIndex)
    }
}

/// Returns a lock guard for the current CPU's `LocalScheduler`.
///
/// The guard holds the per-CPU lock and disables preemption. It is dropped after the caller has
/// finished reading or writing its own scheduler state, typically before returning from a
/// `prepare_*` method.
pub(super) fn local() -> roxy_utils::LockGuard<'static, LocalScheduler> {
    LOCAL.get().lock()
}

/// Initialises the current CPU's scheduler slot.
///
/// Called once per active CPU before the scheduler control loop runs. The BSP does this in
/// `scheduler::initialize`; each AP will do it in `ap_main_2` once SMP is enabled.
pub(super) fn initialize_local() {
    LOCAL.initialize_current(Lock::new(LocalScheduler::new()));
}
