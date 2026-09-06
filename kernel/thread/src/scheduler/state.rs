use alloc::{boxed::Box, vec::Vec};
use core::{
    cell::UnsafeCell,
    sync::atomic::{AtomicBool, Ordering},
};

use roxy_arch::{Architecture, CpuId, CurrentArchitectureBackend, MAX_CPUS};
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
    pub(super) control_context: Option<Box<UnsafeCell<SavedContext>>>,
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
    // Stable boxes keep context pointers valid while other CPUs enqueue threads.
    // Vacant slots preserve ThreadIndex values after a thread is reaped.
    pub(super) entries: Vec<Option<Box<SchedulerEntry>>>,
}

pub(super) struct SchedulerEntry {
    pub(super) thread: Thread,
    pub(super) kind: ThreadKind,
    pub(super) state: ThreadState,
    /// Set while a CPU is running this thread or has reserved it for a pending switch away.
    /// Other CPUs skip it in dispatch and defer its reap until this is cleared at stack handoff.
    pub(super) reserved: Box<AtomicBool>,
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
        }
    }

    pub(super) fn enqueue(&mut self, thread: Thread, kind: ThreadKind) {
        let entry = Some(Box::new(SchedulerEntry {
            thread,
            kind,
            state: ThreadState::Runnable,
            reserved: Box::new(AtomicBool::new(false)),
        }));

        if let Some(slot) = self.entries.iter_mut().find(|slot| slot.is_none()) {
            *slot = entry;
        } else {
            self.entries.push(entry);
        }
    }

    pub(super) fn current_thread_id(&mut self) -> ThreadId {
        let current = local().current.expect("no current thread");
        self.entry(current).thread.id()
    }

    pub(super) fn try_current_thread_id(&self) -> Option<ThreadId> {
        local().current.map(|current| {
            self.entries[current.0]
                .as_ref()
                .expect("current thread slot is vacant")
                .thread
                .id()
        })
    }

    pub(super) fn entry(&mut self, index: ThreadIndex) -> &mut SchedulerEntry {
        self.entries[index.0]
            .as_deref_mut()
            .expect("thread slot is vacant")
    }

    pub(super) fn index_of(&self, thread_id: ThreadId) -> Option<ThreadIndex> {
        self.entries
            .iter()
            .position(|entry| {
                entry
                    .as_ref()
                    .is_some_and(|entry| entry.thread.id() == thread_id)
            })
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

/// Whether each CPU is idle, i.e. currently blocked in `wait_for_interrupt` with no thread to run.
///
/// Each CPU updates only its own slot. The array is shared so a CPU doing an `enqueue`/`wake` can
/// find idle application processors to reschedule via an IPI; `mark_idle`/`wake_idle_aps` keep it
/// consistent. This is cross-CPU readable despite `LocalScheduler.current` being CPU-local.
static IDLE: [AtomicBool; MAX_CPUS] = [const { AtomicBool::new(false) }; MAX_CPUS];

fn cpu_index() -> usize {
    let index = CurrentArchitectureBackend::current_cpu_id().get() as usize;
    assert!(
        index < MAX_CPUS,
        "CPU id {index} exceeds MAX_CPUS ({MAX_CPUS})"
    );
    index
}

/// Marks the current CPU as idle (about to block in `wait_for_interrupt`) or busy (running a
/// thread).
pub(super) fn mark_idle(idle: bool) {
    IDLE[cpu_index()].store(idle, Ordering::Release);
}

/// Wakes every idle application processor so it re-enters its dispatch loop after a thread became
/// runnable.
///
/// Must be called with interrupts disabled. The current CPU is skipped (it is the one performing
/// the enqueue/wake and is not idle itself).
pub(super) fn wake_idle_aps() {
    let current = CurrentArchitectureBackend::current_cpu_id();
    for (index, idle) in IDLE.iter().enumerate() {
        let cpu = CpuId::new(u32::try_from(index).expect("MAX_CPUS fits in u32"));
        if cpu == current || !idle.load(Ordering::Acquire) {
            continue;
        }
        roxy_interrupt::send_reschedule_ipi(cpu);
    }
}
