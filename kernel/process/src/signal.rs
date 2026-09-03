use roxy_arch::{ResumeInfo, SYSCALL_INSTRUCTION_SIZE, UserContext};
use roxy_signal::{DefaultAction, Signal, SignalSet};
use roxy_thread::scheduler;

use crate::{
    ExitStatus, Process, ProcessGroupId, ProcessId, ProcessState, signal_frame,
    table::{PROCESS_TABLE, process_ids_in_group},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SignalAction {
    Default,
    Ignore,
    /// Runs the user handler at `address`.
    ///
    /// `mask` is added to the process mask for the duration of the handler. When `include_siginfo` is
    /// set the handler was installed with `SA_SIGINFO`, so it is invoked as
    /// `(signo, siginfo_t *, ucontext_t *)` with real structures on the signal frame; otherwise
    /// it receives the signal number as its only argument. `restart` indicates that `SA_RESTART`
    /// was set, so an interrupted blocking syscall is re-executed after the handler returns.
    Handler {
        address: u64,
        mask: SignalSet,
        include_siginfo: bool,
        restart: bool,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SignalError {
    NoSuchProcess,
    UnsupportedAction,
}

/// A queued signal and the metadata needed to build its `siginfo_t`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PendingSignal {
    pub(super) signal: Signal,
    /// Process id of the sender; `0` for kernel-originated signals.
    pub(super) sender_pid: u64,
    /// Why the signal was generated; mapped to the ABI `si_code` only when the `siginfo_t` is
    /// serialized onto a frame.
    pub(super) source: SignalSource,
}

/// The origin of a pending signal, kept ABI-neutral by `roxy-process`.
///
/// Converted to the Linux `si_code` integer only at the frame-serialization boundary, so the
/// process layer never depends on an ABI's numeric conventions.
///
/// `Tkill` and `Kernel` are reserved for `tgkill`-directed and kernel-raised signals, which Roxy
/// does not produce yet; only `Process` is currently queued.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(dead_code)]
pub(super) enum SignalSource {
    /// A user process sent it through the process-directed send syscall.
    Process,
    /// A thread-directed `tgkill`/`tkill` sent it (not yet produced by Roxy).
    Tkill,
    /// The kernel itself generated it (exceptions, hardware faults).
    Kernel,
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
    let thread_id = {
        let mut table = PROCESS_TABLE.lock();
        // Called from IRQ context (e.g. terminal ISIG) there may be no "current" thread;
        // use 0 (kernel) as the sender pid in that case.
        let sender_pid = roxy_thread::scheduler::try_current_thread_id()
            .and_then(|tid| table.thread_owners.get(&tid).copied())
            .map_or(0, ProcessId::as_u64);
        let Some(process) = table.processes.get_mut(&process_id) else {
            return Err(SignalError::NoSuchProcess);
        };

        // Set when SIGCONT resumes a stopped process, to wake the parent's waiter only after
        // `process`'s mutable borrow has been released below.
        let mut wake_waiter = false;

        let resume_thread_id = match process.state {
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
                    process.main_thread_id
                }
                // SIGKILL must still terminate a stopped process: queue it and wake the
                // thread so it reaches the return boundary that runs the terminate default.
                Signal::Kill => {
                    process.queue_signal(PendingSignal {
                        signal,
                        sender_pid,
                        source: SignalSource::Kernel,
                    });
                    process.main_thread_id
                }
                // Further stop signals are ignored; everything else stays queued until
                // SIGCONT resumes the process.
                _ => {
                    if matches!(signal.default_action(), DefaultAction::Stop) {
                        return Ok(());
                    }
                    process.queue_signal(PendingSignal {
                        signal,
                        sender_pid,
                        source: SignalSource::Kernel,
                    });
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

                // A signal with no sender (IRQ-generated, e.g. terminal ISIG) is attributed
                // to the kernel; otherwise it is a user-process signal.
                let source = if sender_pid == 0 {
                    SignalSource::Kernel
                } else {
                    SignalSource::Process
                };

                process.queue_signal(PendingSignal {
                    signal,
                    sender_pid,
                    source,
                });
                process.main_thread_id
            }
        };

        if wake_waiter {
            table.wake_state_waiter(process_id);
        }

        resume_thread_id
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

/// Applies one pending signal for the current process.
///
/// Default actions run immediately; handler dispositions build a signal frame on the user stack
/// and return the resume that enters the handler. Must run only where abandoning the current
/// userspace return is safe, exactly once per userspace return boundary.
#[must_use]
pub fn deliver_pending_signal(context: &UserContext, is_interrupted: bool) -> Option<ResumeInfo> {
    let pending = take_pending_unmasked_signal()?;
    let signal = pending.signal;
    let action = signal_action_of(signal);

    match action {
        SignalAction::Ignore => None,
        SignalAction::Default => {
            do_default_action(signal, signal.default_action());

            None
        }
        SignalAction::Handler {
            address,
            mask,
            include_siginfo,
            restart,
        } => {
            // When the handler has SA_RESTART and the interrupted syscall returned EINTR,
            // adjust the saved instruction pointer back to the `syscall` instruction so that
            // after the handler returns (via sigreturn) the CPU re-executes the syscall with
            // the original arguments (still in registers from the syscall entry context).
            let adjusted = if restart && is_interrupted {
                let mut adjusted = *context;
                adjusted.instruction_pointer = adjusted
                    .instruction_pointer
                    .wrapping_sub(SYSCALL_INSTRUCTION_SIZE);
                adjusted
            } else {
                *context
            };

            Some(prepare_handler_resume(
                &adjusted,
                signal,
                address,
                mask,
                include_siginfo,
                pending,
            ))
        }
    }
}

/// Builds the signal frame and signal-mask updates that enter the user handler, returning the
/// `ResumeInfo` that actually resumes into it. Delivery itself completes only when the caller
/// applies that resume to the saved context.
fn prepare_handler_resume(
    context: &UserContext,
    signal: Signal,
    address: u64,
    handler_mask: SignalSet,
    include_siginfo: bool,
    pending: PendingSignal,
) -> ResumeInfo {
    let mut table = PROCESS_TABLE.lock();
    let process = table
        .current_process()
        .expect("current thread has no process");
    let addrspace = process
        .addrspace
        .clone()
        .expect("running process has no address space");

    // The mask active before delivery, snapshot before the handler's mask is merged in.
    let old_mask = process.masked_signals;

    // Skips the 128-byte red zone below the interrupted stack pointer and aligns the frame so
    // the handler entry satisfies the System V stack alignment (`frame % 16 == 8`).
    let frame_base = context
        .stack_pointer
        .checked_sub(128)
        .and_then(|value| value.checked_sub(signal_frame::SIGNAL_FRAME_SIZE as u64))
        .map(|value| value & !0xF)
        .and_then(|value| value.checked_sub(8))
        .expect("user stack has room for a signal frame");
    let frame_bytes = signal_frame::build_bytes(context, old_mask, pending);

    addrspace
        .write_bytes(
            roxy_memory::UserAddress::new(frame_base).expect("aligned frame address is canonical"),
            &frame_bytes,
        )
        .expect("signal frame stack region is mapped");

    process.signal_frames.push(frame_base);
    process.masked_signals |= handler_mask | SignalSet::from_signal(signal);

    // An `SA_SIGINFO` handler receives pointers into its own frame; a plain handler gets the
    // signal number and zeroed arguments (the frame still carries the structures so the layout is
    // uniform, but the handler cannot observe them).
    let arguments = if include_siginfo {
        [
            u64::from(signal.number()),
            frame_base + signal_frame::SIGINFO_OFFSET as u64,
            frame_base + signal_frame::UCONTEXT_OFFSET as u64,
        ]
    } else {
        [u64::from(signal.number()), 0, 0]
    };

    ResumeInfo {
        instruction_pointer: address,
        stack_pointer: frame_base,
        arguments,
    }
}

/// Pops the most recent signal frame for the current process and restores its context.
///
/// Returns `None` when the process has no outstanding signal frame, which is a spurious
/// `sigreturn` rather than missing kernel functionality.
#[must_use]
pub fn pop_signal_frame(context: &UserContext) -> Option<UserContext> {
    let mut table = PROCESS_TABLE.lock();
    let process = table.current_process()?;
    let frame_address = process.signal_frames.pop()?;

    if frame_address != context.stack_pointer {
        // The handler returned to the trampoline with an unexpected stack pointer; refuse to
        // restore instead of trusting a foreign frame.
        process.signal_frames.push(frame_address);

        return None;
    }

    let mut frame = [0u8; signal_frame::SIGNAL_FRAME_SIZE];
    let addrspace = process
        .addrspace
        .clone()
        .expect("running process has no address space");

    addrspace
        .read_bytes(
            roxy_memory::UserAddress::new(frame_address)
                .expect("recorded frame address is canonical"),
            &mut frame,
        )
        .expect("recorded signal frame region is mapped");

    let restored = signal_frame::restore_context(
        &frame[signal_frame::USER_CONTEXT_OFFSET
            ..signal_frame::USER_CONTEXT_OFFSET + signal_frame::USER_CONTEXT_SIZE],
    );

    process.masked_signals = signal_frame::restore_old_mask(&frame);

    Some(restored)
}

/// Replaces one signal disposition and returns the previously installed action.
///
/// # Errors
///
/// Returns an error when attempting to ignore `SIGKILL` or `SIGSTOP`.
pub fn replace_signal_action(
    signal: Signal,
    action: SignalAction,
) -> Result<SignalAction, SignalError> {
    if matches!(signal, Signal::Kill | Signal::Stop) && matches!(action, SignalAction::Ignore) {
        return Err(SignalError::UnsupportedAction);
    }

    let mut table = PROCESS_TABLE.lock();
    let process = table
        .current_process()
        .expect("current thread has no process");

    Ok(process.replace_signal_action(signal, action))
}

/// Returns the current process's disposition for `signal`.
#[must_use]
pub fn signal_action_of(signal: Signal) -> SignalAction {
    let mut table = PROCESS_TABLE.lock();
    let process = table
        .current_process()
        .expect("current thread has no process");

    process.signal_action_of(signal)
}

/// Replaces the current process's signal mask.
///
/// `SIGKILL` and `SIGSTOP` are always removed because Unix does not permit masking them.
/// Returns the mask that was active before replacement.
pub fn replace_masked_signals(signals: SignalSet) -> SignalSet {
    let mut table = PROCESS_TABLE.lock();
    let process = table
        .current_process()
        .expect("current thread has no process");

    process.replace_masked_signals(signals)
}

/// Returns the signals currently blocked by the current process.
#[must_use]
pub fn currently_blocked_signals() -> SignalSet {
    let mut table = PROCESS_TABLE.lock();
    let process = table
        .current_process()
        .expect("current thread has no process");

    process.masked_signals
}

/// Adds signals to the current process's signal mask and returns the previous mask.
#[must_use]
pub fn block_signals(signals: SignalSet) -> SignalSet {
    update_masked_signals(|masked| masked.insert(signals))
}

/// Removes signals from the current process's signal mask and returns the previous mask.
#[must_use]
pub fn unblock_signals(signals: SignalSet) -> SignalSet {
    update_masked_signals(|masked| masked.remove(signals))
}

/// Updates the current process's mask while holding the process-table lock.
///
/// `update` receives the current mask and mutates it in place. The returned set is the mask
/// that was active before `update` ran; unmaskable signals are removed before the new mask is
/// published.
fn update_masked_signals(update: impl FnOnce(&mut SignalSet)) -> SignalSet {
    let mut table = PROCESS_TABLE.lock();
    let process = table
        .current_process()
        .expect("current thread has no process");
    let old_mask = process.masked_signals;

    update(&mut process.masked_signals);
    process.masked_signals = filter_unmaskable_signals(process.masked_signals);

    old_mask
}

#[must_use]
fn filter_unmaskable_signals(signals: SignalSet) -> SignalSet {
    signals - (SignalSet::KILL | SignalSet::STOP)
}

/// Reports whether the current process has a pending signal that its mask permits.
#[must_use]
pub fn has_pending_signal() -> bool {
    let mut table = PROCESS_TABLE.lock();
    let Some(process) = table.current_process() else {
        return false;
    };

    process.has_pending_signal()
}

fn take_pending_unmasked_signal() -> Option<PendingSignal> {
    let mut table = PROCESS_TABLE.lock();
    let process = table.current_process()?;

    process.take_latest_signal()
}

fn do_default_action(signal: Signal, action: DefaultAction) {
    match action {
        DefaultAction::Terminate => crate::exit_current(ExitStatus::signaled(signal)),
        DefaultAction::Stop => stop_current(signal),
        // SIGCONT's default action is handled by `send_signal` (it resumes a stopped process
        // directly); a queued Continue would only arise from a handler disposition, which is
        // never delivered through this default-action path.
        DefaultAction::Continue | DefaultAction::Ignore => {}
        DefaultAction::Unsupported => {
            unreachable!("unsupported signal actions cannot be queued")
        }
    }
}

/// Suspends the current process: records its stopped state, wakes any parent waiting with
/// WUNTRACED, and blocks the current thread until SIGCONT resumes it.
///
/// Runs on the process's own thread at a userspace return boundary, so interrupts are disabled
/// and blocking here is safe. `send_signal` clears the stopped state and wakes the thread on
/// SIGCONT; the thread then returns from the block and the interrupted syscall completes.
fn stop_current(signal: Signal) {
    let block = {
        let mut table = PROCESS_TABLE.lock();
        let process_id = table.current_process_id();
        let process = table
            .processes
            .get_mut(&process_id)
            .expect("current process");
        process.state = ProcessState::Stopped(signal);
        table.wake_state_waiter(process_id);
        scheduler::prepare_block_current()
    };
    block.perform();
}

impl Process {
    fn signal_action_of(&self, signal: Signal) -> SignalAction {
        self.signal_actions
            .get(&signal)
            .copied()
            .unwrap_or(SignalAction::Default)
    }

    fn replace_signal_action(&mut self, signal: Signal, action: SignalAction) -> SignalAction {
        let old_action = self
            .signal_actions
            .insert(signal, action)
            .unwrap_or(SignalAction::Default);

        if matches!(action, SignalAction::Ignore) {
            self.pending_signals
                .retain(|pending| pending.signal != signal);
        }

        old_action
    }
}

impl Process {
    fn queue_signal(&mut self, pending: PendingSignal) {
        self.pending_signals.push(pending);
    }

    fn take_latest_signal(&mut self) -> Option<PendingSignal> {
        let index = self.pending_signals.iter().rposition(|pending| {
            !self
                .masked_signals
                .contains(SignalSet::from_signal(pending.signal))
        })?;

        Some(self.pending_signals.remove(index))
    }

    fn has_pending_signal(&self) -> bool {
        self.pending_signals.iter().any(|pending| {
            !self
                .masked_signals
                .contains(SignalSet::from_signal(pending.signal))
        })
    }

    fn replace_masked_signals(&mut self, signals: SignalSet) -> SignalSet {
        let signals = filter_unmaskable_signals(signals);

        core::mem::replace(&mut self.masked_signals, signals)
    }
}

#[cfg(feature = "kernel-test")]
mod tests {

    use roxy_memory::statistics;
    use roxy_signal::{Signal, SignalSet};
    use roxy_test::kernel_test;
    use roxy_thread::Thread;
    use roxy_vm::AddrSpace;

    use super::{PendingSignal, Process, SignalAction, SignalSource, filter_unmaskable_signals};

    fn pending(signal: Signal) -> PendingSignal {
        PendingSignal {
            signal,
            sender_pid: 1,
            source: SignalSource::Process,
        }
    }

    kernel_test!(
        "roxy-process::signal-actions-default-and-ignore",
        signal_actions_default_and_ignore,
        {
            let baseline = statistics().allocated_frames;
            let address_space = AddrSpace::new().unwrap().into_handle();
            let thread = Thread::new(unused_thread).unwrap();
            let mut process =
                Process::new(thread.id(), address_space.clone(), roxy_fd::FdTable::new());

            assert_eq!(
                process.signal_action_of(Signal::Interrupt),
                SignalAction::Default
            );
            process.queue_signal(pending(Signal::Interrupt));
            assert_eq!(
                process.replace_signal_action(Signal::Interrupt, SignalAction::Ignore),
                SignalAction::Default
            );
            assert_eq!(process.take_latest_signal(), None);
            assert_eq!(
                process.replace_signal_action(Signal::Interrupt, SignalAction::Default),
                SignalAction::Ignore
            );
            assert_eq!(
                process.signal_actions.get(&Signal::Interrupt),
                Some(&SignalAction::Default)
            );
            drop(process);
            drop(thread);
            drop(address_space);
            assert_eq!(statistics().allocated_frames, baseline);
        }
    );

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
            assert_eq!(
                process.replace_masked_signals(SignalSet::from_signal(Signal::Interrupt)),
                SignalSet::empty()
            );

            assert_eq!(
                process.take_latest_signal(),
                Some(pending(Signal::Terminate))
            );
            assert_eq!(process.take_latest_signal(), None);
            assert_eq!(
                process.replace_masked_signals(SignalSet::empty()),
                SignalSet::from_signal(Signal::Interrupt)
            );
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

            process.replace_masked_signals(
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
