//! Per-thread signal masks and the pending queues they gate.
//!
//! The main thread's mask is the process mask (`Process::masked_signals`); other threads carry
//! their own mask in `Process::thread_masks`. Thread-directed signals land in
//! `Process::thread_pending`, keyed by thread id. The `Process` pending-queue methods live here
//! too because they are the state these masks gate.

use roxy_signal::SignalSet;
use roxy_thread::{ThreadId, scheduler};

use crate::{Process, signal::PendingSignal, table::PROCESS_TABLE};

/// Replaces the current thread's signal mask.
///
/// `SIGKILL` and `SIGSTOP` are always removed because Unix does not permit masking them.
/// Returns the mask that was active before replacement.
#[must_use]
pub fn replace_masked_signals(signals: SignalSet) -> SignalSet {
    with_current_thread_mask(|process, thread_id| {
        let old = process.effective_mask(thread_id);
        process.set_thread_mask(thread_id, signals);
        old
    })
}

/// Returns the signals currently blocked by the current thread.
#[must_use]
pub fn currently_blocked_signals() -> SignalSet {
    with_current_thread_mask(|process, thread_id| process.effective_mask(thread_id))
}

/// Adds signals to the current thread's signal mask and returns the previous mask.
#[must_use]
pub fn block_signals(signals: SignalSet) -> SignalSet {
    update_thread_mask(|masked| masked.insert(signals))
}

/// Removes signals from the current thread's signal mask and returns the previous mask.
#[must_use]
pub fn unblock_signals(signals: SignalSet) -> SignalSet {
    update_thread_mask(|masked| masked.remove(signals))
}

/// Updates the current thread's mask while holding the process-table lock.
///
/// `update` receives the current mask and mutates it in place. The returned set is the mask
/// that was active before `update` ran; unmaskable signals are removed before the new mask is
/// published.
fn update_thread_mask(update: impl FnOnce(&mut SignalSet)) -> SignalSet {
    with_current_thread_mask(|process, thread_id| {
        let old_mask = process.effective_mask(thread_id);

        let mut masked = old_mask;
        update(&mut masked);
        process.set_thread_mask(thread_id, masked);

        old_mask
    })
}

/// Runs `f` against the current thread's process under the process-table lock.
fn with_current_thread_mask<R>(f: impl FnOnce(&mut Process, ThreadId) -> R) -> R {
    let mut table = PROCESS_TABLE.lock();
    let thread_id = scheduler::current_thread_id();
    let process = table
        .current_process()
        .expect("current thread has no process");

    f(process, thread_id)
}

#[must_use]
fn filter_unmaskable_signals(signals: SignalSet) -> SignalSet {
    signals - (SignalSet::KILL | SignalSet::STOP)
}

/// Reports whether `thread_id` is a live thread of the process that owns the current thread.
#[must_use]
pub fn thread_belongs_to_current_process(thread_id: ThreadId) -> bool {
    let current = scheduler::current_thread_id();

    if thread_id == current {
        return true;
    }

    let table = PROCESS_TABLE.lock();
    let Some(owner) = table.thread_owners.get(&thread_id).copied() else {
        return false;
    };
    let Some(current_owner) = table.thread_owners.get(&current).copied() else {
        return false;
    };

    owner == current_owner
}

impl Process {
    pub(super) fn queue_signal(&mut self, pending: PendingSignal) {
        self.pending_signals.push(pending);
    }

    pub(super) fn queue_thread_signal(&mut self, thread_id: ThreadId, pending: PendingSignal) {
        self.thread_pending
            .entry(thread_id)
            .or_default()
            .push(pending);
    }

    /// The effective signal mask of `thread_id`: the main thread uses the process mask, other
    /// threads their own (inherited) mask.
    pub(super) fn effective_mask(&self, thread_id: ThreadId) -> SignalSet {
        if thread_id == self.main_thread_id {
            self.masked_signals
        } else {
            self.thread_masks
                .get(&thread_id)
                .copied()
                .unwrap_or(SignalSet::empty())
        }
    }

    /// Sets the effective signal mask of `thread_id`; unmaskable signals are stripped first.
    pub(super) fn set_thread_mask(&mut self, thread_id: ThreadId, signals: SignalSet) {
        let signals = filter_unmaskable_signals(signals);

        if thread_id == self.main_thread_id {
            self.masked_signals = signals;
        } else {
            self.thread_masks.insert(thread_id, signals);
        }
    }

    /// Pops the most recent unmasked pending signal for `thread_id`, scanning the thread's
    /// targeted queue first, then the process queue.
    pub(super) fn take_pending_for(&mut self, thread_id: ThreadId) -> Option<PendingSignal> {
        let mask = self.effective_mask(thread_id);

        if let Some(queue) = self.thread_pending.get_mut(&thread_id) {
            let index = queue
                .iter()
                .rposition(|pending| !mask.contains(SignalSet::from_signal(pending.signal)));
            if let Some(index) = index {
                return Some(queue.remove(index));
            }
        }

        let index = self
            .pending_signals
            .iter()
            .rposition(|pending| !mask.contains(SignalSet::from_signal(pending.signal)))?;
        Some(self.pending_signals.remove(index))
    }

    /// Reports whether `thread_id` has any pending unmasked signal (thread-targeted or process).
    pub(super) fn has_pending_for(&self, thread_id: ThreadId) -> bool {
        let mask = self.effective_mask(thread_id);
        let unmasked =
            |pending: &PendingSignal| !mask.contains(SignalSet::from_signal(pending.signal));

        self.thread_pending
            .get(&thread_id)
            .is_some_and(|queue| queue.iter().any(unmasked))
            || self.pending_signals.iter().any(unmasked)
    }

    /// Pops the most recent pending signal of `thread_id` whose number is in `set`, whether or not
    /// the thread currently blocks it. Scans the thread queue first.
    pub(super) fn take_matching(
        &mut self,
        thread_id: ThreadId,
        set: SignalSet,
    ) -> Option<PendingSignal> {
        let matches =
            |pending: &PendingSignal| set.contains(SignalSet::from_signal(pending.signal));

        let thread_hit = self
            .thread_pending
            .get(&thread_id)
            .and_then(|queue| queue.iter().rposition(matches));
        if let Some(index) = thread_hit {
            let queue = self
                .thread_pending
                .get_mut(&thread_id)
                .expect("not mutated");
            return Some(queue.remove(index));
        }

        let index = self.pending_signals.iter().rposition(matches)?;
        Some(self.pending_signals.remove(index))
    }

    // The main-thread conveniences below mirror the historical process-wide view and are used by
    // the tests and by code that reasons about a single-threaded process.

    #[cfg_attr(not(feature = "kernel-test"), allow(dead_code))]
    pub(super) fn take_latest_signal(&mut self) -> Option<PendingSignal> {
        self.take_pending_for(self.main_thread_id)
    }

    #[cfg_attr(not(feature = "kernel-test"), allow(dead_code))]
    pub(super) fn has_pending_signal(&self) -> bool {
        self.has_pending_for(self.main_thread_id)
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_memory::statistics;
    use roxy_signal::{Signal, SignalSet};
    use roxy_test::kernel_test;
    use roxy_thread::Thread;
    use roxy_vm::AddrSpace;

    use crate::{
        Process,
        signal::{PendingSignal, SignalSource},
    };

    use super::filter_unmaskable_signals;

    fn pending(signal: Signal) -> PendingSignal {
        PendingSignal {
            signal,
            sender_pid: 1,
            source: SignalSource::Process,
            value: None,
        }
    }

    kernel_test!(
        "roxy-process::pending-signals-keep-order",
        pending_signals_keep_order,
        {
            let baseline = statistics().allocated_frames;
            let address_space = AddrSpace::new().unwrap().into_handle();
            let thread = Thread::new(unused_thread).unwrap();
            let mut process =
                Process::new(thread.id(), address_space.clone(), roxy_fd::FdTable::new());

            assert!(process.masked_signals.is_empty());
            process.queue_signal(pending(Signal::Terminate));
            process.queue_signal(pending(Signal::Interrupt));

            assert_eq!(
                process.take_latest_signal(),
                Some(pending(Signal::Interrupt))
            );
            assert_eq!(
                process.take_latest_signal(),
                Some(pending(Signal::Terminate))
            );
            assert_eq!(process.take_latest_signal(), None);
            drop(process);
            drop(thread);
            drop(address_space);
            assert_eq!(statistics().allocated_frames, baseline);
        }
    );

    kernel_test!(
        "roxy-process::masked-signals-stay-pending",
        masked_signals_stay_pending,
        {
            let baseline = statistics().allocated_frames;
            let address_space = AddrSpace::new().unwrap().into_handle();
            let thread = Thread::new(unused_thread).unwrap();
            let mut process =
                Process::new(thread.id(), address_space.clone(), roxy_fd::FdTable::new());

            process.queue_signal(pending(Signal::Terminate));
            process.queue_signal(pending(Signal::Interrupt));
            process.set_thread_mask(
                process.main_thread_id,
                SignalSet::from_signal(Signal::Interrupt),
            );
            assert_eq!(
                process.masked_signals,
                SignalSet::from_signal(Signal::Interrupt)
            );

            assert_eq!(
                process.take_latest_signal(),
                Some(pending(Signal::Terminate))
            );
            assert_eq!(process.take_latest_signal(), None);
            assert!(!process.has_pending_signal());
            process.set_thread_mask(process.main_thread_id, SignalSet::empty());
            assert!(process.masked_signals.is_empty());
            assert!(process.has_pending_signal());
            assert_eq!(
                process.take_latest_signal(),
                Some(pending(Signal::Interrupt))
            );
            drop(process);
            drop(thread);
            drop(address_space);
            assert_eq!(statistics().allocated_frames, baseline);
        }
    );

    kernel_test!(
        "roxy-process::kill-and-stop-cannot-be-masked",
        kill_and_stop_cannot_be_masked,
        {
            let baseline = statistics().allocated_frames;
            let address_space = AddrSpace::new().unwrap().into_handle();
            let thread = Thread::new(unused_thread).unwrap();
            let mut process =
                Process::new(thread.id(), address_space.clone(), roxy_fd::FdTable::new());

            process.set_thread_mask(
                process.main_thread_id,
                SignalSet::KILL | SignalSet::STOP | SignalSet::from_signal(Signal::Terminate),
            );

            assert_eq!(
                process.masked_signals,
                SignalSet::from_signal(Signal::Terminate)
            );
            drop(process);
            drop(thread);
            drop(address_space);
            assert_eq!(statistics().allocated_frames, baseline);
        }
    );

    kernel_test!(
        "roxy-process::filter-unmaskable-signals",
        filter_unmaskable,
        {
            assert_eq!(
                filter_unmaskable_signals(SignalSet::KILL | SignalSet::INTERRUPT | SignalSet::STOP),
                SignalSet::from_signal(Signal::Interrupt)
            );
        }
    );

    fn unused_thread() -> ! {
        panic!("unused process test thread started")
    }
}
