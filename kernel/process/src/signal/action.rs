//! Signal dispositions: the action model, replacing them, and looking them up.

use roxy_signal::{Signal, SignalSet};

use crate::{Process, signal::SignalError, table::PROCESS_TABLE};

/// How a signal is handled: `Default`, `Ignore`, or `Handler` at `address`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SignalAction {
    Default,
    Ignore,
    /// Runs the user handler at `address`.
    ///
    /// `mask` is added to the process mask for the duration of the handler. When `include_siginfo` is
    /// set the handler was installed with `SA_SIGINFO`, so it is invoked as
    /// `(signo, siginfo_t *, null)` with the record on the signal frame; otherwise it receives the
    /// signal number as its only argument. `restart` indicates that `SA_RESTART` was set, so an
    /// interrupted blocking syscall is re-executed after the handler returns.
    Handler {
        address: u64,
        mask: SignalSet,
        include_siginfo: bool,
        restart: bool,
    },
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

impl Process {
    pub(super) fn signal_action_of(&self, signal: Signal) -> SignalAction {
        self.signal_actions
            .get(&signal)
            .copied()
            .unwrap_or(SignalAction::Default)
    }

    pub(super) fn replace_signal_action(
        &mut self,
        signal: Signal,
        action: SignalAction,
    ) -> SignalAction {
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

    use super::SignalAction;

    fn pending(signal: Signal) -> PendingSignal {
        PendingSignal {
            signal,
            sender_pid: 1,
            source: SignalSource::Process,
            value: None,
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

    fn unused_thread() -> ! {
        panic!("unused process test thread started")
    }
}
