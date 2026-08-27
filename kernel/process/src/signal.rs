use alloc::vec::Vec;

use roxy_signal::{DefaultAction, Signal};
use roxy_thread::scheduler;

use crate::{ExitStatus, Process, ProcessId, ProcessState, table::PROCESS_TABLE};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SignalAction {
    Default,
    Ignore,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SignalError {
    NoSuchProcess,
    UnsupportedAction,
}

/// Queues a signal for a running process and wakes it if it is blocked.
///
/// The target consumes the queued signal at a userspace return boundary. Sending never exits the
/// target directly because its thread may still be executing on its own kernel stack.
///
/// # Errors
///
/// Returns an error when the target process does not exist or the effective default action is not
/// implemented.
pub fn send_signal(process_id: ProcessId, signal: Signal) -> Result<(), SignalError> {
    let thread_id = {
        let mut table = PROCESS_TABLE.lock();
        let Some(process) = table.processes.get_mut(&process_id) else {
            return Err(SignalError::NoSuchProcess);
        };

        if !matches!(process.state, ProcessState::Running) {
            return Err(SignalError::NoSuchProcess);
        }

        match process.signal_action_of(signal) {
            SignalAction::Ignore => return Ok(()),
            SignalAction::Default
                if matches!(signal.default_action(), DefaultAction::Unsupported) =>
            {
                return Err(SignalError::UnsupportedAction);
            }
            SignalAction::Default => {}
        }

        process.queue_signal(signal);
        process.main_thread_id
    };

    let _ = scheduler::wake_unconditionally(thread_id);

    Ok(())
}

/// Applies one pending signal action for the current process.
///
/// This function must run only where abandoning the current userspace return is safe.
pub fn process_latest_signal() {
    let signal = take_latest_signal();
    let Some(signal) = signal else {
        return;
    };
    let action = signal_action_of(signal);

    process_signal(signal, action);
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
pub fn replace_masked_signals(signals: Vec<Signal>) -> Vec<Signal> {
    let mut table = PROCESS_TABLE.lock();
    let process = table
        .current_process()
        .expect("current thread has no process");

    process.replace_masked_signals(signals)
}

/// Returns the signals currently blocked by the current process.
#[must_use]
pub fn currently_blocked_signals() -> Vec<Signal> {
    let mut table = PROCESS_TABLE.lock();
    let process = table
        .current_process()
        .expect("current thread has no process");

    process.masked_signals.clone()
}

/// Adds signals to the current process's signal mask and returns the previous mask.
#[must_use]
pub fn block_signals(signals: Vec<Signal>) -> Vec<Signal> {
    update_masked_signals(|masked| {
        for signal in signals {
            if !masked.contains(&signal) {
                masked.push(signal);
            }
        }
    })
}

/// Removes signals from the current process's signal mask and returns the previous mask.
#[must_use]
pub fn unblock_signals(signals: &[Signal]) -> Vec<Signal> {
    update_masked_signals(|masked| masked.retain(|signal| !signals.contains(signal)))
}

/// Updates the current process's mask while holding the process-table lock.
///
/// `update` receives the current mask and mutates it in place. The returned vector is the mask
/// that was active before `update` ran; unmaskable signals are removed before the new mask is
/// published.
fn update_masked_signals(update: impl FnOnce(&mut Vec<Signal>)) -> Vec<Signal> {
    let mut table = PROCESS_TABLE.lock();
    let process = table
        .current_process()
        .expect("current thread has no process");
    let old_mask = process.masked_signals.clone();

    update(&mut process.masked_signals);
    process.masked_signals =
        filter_unmaskable_signals(core::mem::take(&mut process.masked_signals));

    old_mask
}

#[must_use]
fn filter_unmaskable_signals(mut signals: Vec<Signal>) -> Vec<Signal> {
    signals.retain(|signal| !matches!(signal, Signal::Kill | Signal::Stop));

    signals
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

fn take_latest_signal() -> Option<Signal> {
    let mut table = PROCESS_TABLE.lock();
    let process = table.current_process()?;

    process.take_latest_signal()
}

fn process_signal(signal: Signal, action: SignalAction) {
    match action {
        SignalAction::Ignore => unreachable!("ignored signals cannot reach delivery"),
        SignalAction::Default => do_default_action(signal, signal.default_action()),
    }
}

fn do_default_action(signal: Signal, action: DefaultAction) {
    match action {
        DefaultAction::Terminate => crate::exit_current(ExitStatus::signaled(signal)),
        DefaultAction::Ignore => {}
        DefaultAction::Unsupported => {
            unreachable!("unsupported signal actions cannot be queued")
        }
    }
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
            self.pending_signals.retain(|pending| *pending != signal);
        }

        old_action
    }
}

impl Process {
    fn queue_signal(&mut self, signal: Signal) {
        self.pending_signals.push(signal);
    }

    fn take_latest_signal(&mut self) -> Option<Signal> {
        let index = self
            .pending_signals
            .iter()
            .rposition(|signal| !self.masked_signals.contains(signal))?;

        Some(self.pending_signals.remove(index))
    }

    fn has_pending_signal(&self) -> bool {
        self.pending_signals
            .iter()
            .any(|signal| !self.masked_signals.contains(signal))
    }

    fn replace_masked_signals(&mut self, signals: Vec<Signal>) -> Vec<Signal> {
        let signals = filter_unmaskable_signals(signals);

        core::mem::replace(&mut self.masked_signals, signals)
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use alloc::{vec, vec::Vec};

    use roxy_memory::statistics;
    use roxy_signal::Signal;
    use roxy_test::kernel_test;
    use roxy_thread::Thread;
    use roxy_vm::AddrSpace;

    use super::{Process, SignalAction, filter_unmaskable_signals};

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
            process.queue_signal(Signal::Interrupt);
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
            process.queue_signal(Signal::Terminate);
            process.queue_signal(Signal::Interrupt);

            assert_eq!(process.take_latest_signal(), Some(Signal::Interrupt));
            assert_eq!(process.take_latest_signal(), Some(Signal::Terminate));
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

            process.queue_signal(Signal::Terminate);
            process.queue_signal(Signal::Interrupt);
            assert_eq!(
                process.replace_masked_signals(vec![Signal::Interrupt]),
                Vec::new()
            );

            assert_eq!(process.take_latest_signal(), Some(Signal::Terminate));
            assert_eq!(process.take_latest_signal(), None);
            assert_eq!(
                process.replace_masked_signals(Vec::new()),
                vec![Signal::Interrupt]
            );
            assert_eq!(process.take_latest_signal(), Some(Signal::Interrupt));
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

            process.replace_masked_signals(vec![Signal::Kill, Signal::Stop, Signal::Terminate]);

            assert_eq!(process.masked_signals, vec![Signal::Terminate]);
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
                filter_unmaskable_signals(vec![Signal::Kill, Signal::Interrupt, Signal::Stop]),
                vec![Signal::Interrupt]
            );
        }
    );

    fn unused_thread() -> ! {
        panic!("unused process test thread started")
    }
}
