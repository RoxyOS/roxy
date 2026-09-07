//! Queuing signals for a process, a process group, or a specific thread.
//!
//! Also owns the records of a queued signal (`PendingSignal`, its `SignalSource`) and the send
//! error type, because a send is where they are produced and where they fail.

use roxy_signal::{DefaultAction, Signal};
use roxy_thread::{ThreadId, scheduler};

use crate::{
    ProcessGroupId, ProcessId, ProcessState,
    signal::SignalAction,
    table::{PROCESS_TABLE, process_ids_in_group},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SignalError {
    NoSuchProcess,
    UnsupportedAction,
}

/// A queued signal and the metadata needed to build its `siginfo_t`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PendingSignal {
    pub(crate) signal: Signal,
    /// Process id of the sender; `0` for kernel-originated signals.
    pub(crate) sender_pid: u64,
    /// Why the signal was generated; mapped to the ABI `si_code` only when the `siginfo_t` is
    /// serialized onto a frame.
    pub(crate) source: SignalSource,
    /// Userspace payload reported to `SA_SIGINFO` handlers through `si_value`. Only set for
    /// kernel-raised timer signals carrying a POSIX `sigval`.
    pub(crate) value: Option<u64>,
}

/// The origin of a pending signal, kept ABI-neutral by `roxy-process`.
///
/// Converted to the Linux `si_code` integer only at the frame-serialization boundary, so the
/// process layer never depends on an ABI's numeric conventions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(dead_code)]
pub(crate) enum SignalSource {
    /// A user process sent it through the process-directed send syscall.
    Process,
    /// A signal directed at one specific thread (not yet produced by Roxy).
    Tkill,
    /// The kernel itself generated it (exceptions, hardware faults).
    Kernel,
    /// A POSIX timer generated it; reported with `si_code == SI_TIMER` and the timer's `sigval`.
    Timer,
}

/// Queues a signal for a process and wakes its thread when the process should resume.
///
/// A stopped process is resumed by SIGCONT and terminated by SIGKILL; any other signal is
/// queued but does not wake the stopped thread (it is delivered after continuation). The
/// target consumes the queued signal at a userspace return boundary. Sending never exits the
/// target directly because its thread may still be executing on its own kernel stack.
///
/// # Errors
///
/// Returns an error when the target process does not exist or the effective default action is
/// not implemented.
pub fn send_signal(process_id: ProcessId, signal: Signal) -> Result<(), SignalError> {
    let (sender_pid, source) = sender_identity();

    send_signal_impl(process_id, None, signal, source, sender_pid, None)
}

/// Queues a POSIX-timer-expiration signal for `process_id` with the timer's `sigval` payload.
///
/// Reports `si_code == SI_TIMER` and `si_value == value` to `SA_SIGINFO` handlers. The sender
/// is attributed to the kernel (`sender_pid == 0`). Shares all process-state and disposition
/// rules with [`send_signal`].
///
/// # Errors
///
/// Returns an error when the target process does not exist or the effective default action is
/// not implemented.
#[allow(clippy::needless_pass_by_value)]
pub fn send_timer_signal(
    process_id: ProcessId,
    signal: Signal,
    value: u64,
) -> Result<(), SignalError> {
    send_signal_impl(
        process_id,
        None,
        signal,
        SignalSource::Timer,
        0,
        Some(value),
    )
}

/// Queues a signal for a specific thread of the target process.
///
/// The signal lands in that thread's own per-thread pending queue and wakes the thread. It stays
/// pending until the thread unblocks and takes it at a userspace return boundary, or until it
/// explicitly consumes the signal. The sender and disposition rules otherwise follow
/// [`send_signal`].
///
/// # Errors
///
/// Returns an error when the target thread (or its process) does not exist.
pub fn send_thread_signal(thread_id: ThreadId, signal: Signal) -> Result<(), SignalError> {
    let (sender_pid, source) = sender_identity();
    let process_id = process_id_of_thread(thread_id).ok_or(SignalError::NoSuchProcess)?;

    send_signal_impl(
        process_id,
        Some(thread_id),
        signal,
        source,
        sender_pid,
        None,
    )
}

/// Queues a POSIX-timer-expiration signal for a specific thread, carrying the timer's `sigval`
/// payload (reported as `si_code == SI_TIMER`).
///
/// The signal lands in that thread's own per-thread pending queue, attributed to the kernel, and
/// wakes the thread. It is the current (and only) producer of a thread-directed signal that
/// carries an opaque value word.
///
/// # Errors
///
/// Returns an error when the target thread (or its process) does not exist.
///
/// Note: as with any thread-directed delivery this respects the process's disposition, so an
/// application must not ignore the signal number this timer delivers.
#[allow(clippy::needless_pass_by_value)]
pub fn send_thread_timer_signal(
    thread_id: ThreadId,
    signal: Signal,
    value: u64,
) -> Result<(), SignalError> {
    let process_id = process_id_of_thread(thread_id).ok_or(SignalError::NoSuchProcess)?;

    send_signal_impl(
        process_id,
        Some(thread_id),
        signal,
        SignalSource::Timer,
        0,
        Some(value),
    )
}

/// Resolves the process owning `thread_id`, if that thread is still live.
fn process_id_of_thread(thread_id: ThreadId) -> Option<ProcessId> {
    let table = PROCESS_TABLE.lock();
    table.thread_owners.get(&thread_id).copied()
}

/// Resolves the identity attributed to an ordinary [`send_signal`] delivery.
///
/// Called from IRQ context (e.g. terminal ISIG) there may be no "current" thread; in that case
/// the sender is 0 (kernel) and the signal is attributed to the kernel.
fn sender_identity() -> (u64, SignalSource) {
    let table = PROCESS_TABLE.lock();
    let sender_pid = roxy_thread::scheduler::try_current_thread_id()
        .and_then(|tid| table.thread_owners.get(&tid).copied())
        .map_or(0, ProcessId::as_u64);
    let source = if sender_pid == 0 {
        SignalSource::Kernel
    } else {
        SignalSource::Process
    };

    (sender_pid, source)
}

/// Shared delivery core for a process-directed or thread-targeted signal.
///
/// When `target_thread` is `Some`, the signal lands in that thread's per-thread pending queue
/// and that thread is woken. Otherwise the signal is queued process-wide and the target thread
/// is chosen by the process table (preferring the main thread). In both cases the wake happens
/// **after** releasing the process-table lock, so scheduling never runs while the table is held.
fn send_signal_impl(
    process_id: ProcessId,
    target_thread: Option<ThreadId>,
    signal: Signal,
    source: SignalSource,
    sender_pid: u64,
    value: Option<u64>,
) -> Result<(), SignalError> {
    let thread_id = {
        let mut table = PROCESS_TABLE.lock();

        let Some(process) = table.processes.get_mut(&process_id) else {
            return Err(SignalError::NoSuchProcess);
        };

        // Set when SIGCONT resumes a stopped process, to wake the parent's waiter only after
        // `process`'s mutable borrow has been released below.
        let mut wake_waiter = false;

        let pending = PendingSignal {
            signal,
            sender_pid,
            source,
            value,
        };

        match process.state {
            // Reaped or exiting processes are no longer reachable.
            ProcessState::Exited(_) | ProcessState::Exiting(_) => {
                return Err(SignalError::NoSuchProcess);
            }
            ProcessState::Stopped(_) => match signal {
                // SIGCONT resumes the process: clear the stopped state so the default action
                // returns the thread to the syscall return it was stopped in.
                Signal::Continue => {
                    process.state = ProcessState::Running;
                    // Record the continuation so a parent waiting with WCONTINUED can observe
                    // this single resumption, and wake that waiter to report it.
                    process.continued = true;
                    wake_waiter = true;
                }
                // SIGKILL must still terminate a stopped process: queue it and wake the
                // thread so it reaches the return boundary that runs the terminate default.
                Signal::Kill => {
                    if let Some(target) = target_thread {
                        process.queue_thread_signal(target, pending);
                    } else {
                        process.queue_signal(pending);
                    }
                }
                // Further stop signals are ignored; everything else stays queued until
                // SIGCONT resumes the process.
                _ => {
                    if matches!(signal.default_action(), DefaultAction::Stop) {
                        return Ok(());
                    }
                    if let Some(target) = target_thread {
                        process.queue_thread_signal(target, pending);
                    } else {
                        process.queue_signal(pending);
                    }
                    return Ok(());
                }
            },
            ProcessState::Running => {
                // SIGCONT's default action on a running process is a no-op.
                if signal == Signal::Continue
                    && matches!(process.signal_action_of(signal), SignalAction::Default)
                {
                    return Ok(());
                }

                match process.signal_action_of(signal) {
                    SignalAction::Ignore => return Ok(()),
                    SignalAction::Default
                        if matches!(signal.default_action(), DefaultAction::Unsupported) =>
                    {
                        return Err(SignalError::UnsupportedAction);
                    }
                    SignalAction::Handler { .. } | SignalAction::Default => {}
                }

                if let Some(target) = target_thread {
                    process.queue_thread_signal(target, pending);
                } else {
                    process.queue_signal(pending);
                }
            }
        }

        if wake_waiter {
            table.wake_state_waiter(process_id);
        }

        // Prefer the main thread when it is still scheduled; otherwise deliver to any remaining
        // thread of the process (e.g. when the main thread has already reaped).
        target_thread.unwrap_or_else(|| table.signal_target_thread(process_id))
    };

    let _ = scheduler::wake_unconditionally(thread_id);

    Ok(())
}

/// Sends a signal to every process currently in the given process group.
///
/// The group membership snapshot is taken under the process-table lock before any delivery, so a
/// process that exits mid-delivery is simply skipped by `send_signal`.
pub fn send_signal_to_pgid(pgid: ProcessGroupId, signal: Signal) {
    let targets = process_ids_in_group(pgid);

    for target in targets {
        let _ = send_signal(target, signal);
    }
}
