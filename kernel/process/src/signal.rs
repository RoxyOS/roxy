use roxy_signal::{Signal, SignalAction};
use roxy_thread::scheduler;

use crate::{ExitStatus, Process, ProcessId, ProcessState, table::PROCESS_TABLE};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SignalError {
    NoSuchProcess,
}

/// Queues a signal for a running process and wakes it if it is blocked.
///
/// The target consumes the queued signal at a userspace return boundary. Sending never exits the
/// target directly because its thread may still be executing on its own kernel stack.
pub fn send_signal(process_id: ProcessId, signal: Signal) -> Result<(), SignalError> {
    let thread_id = {
        let mut table = PROCESS_TABLE.lock();
        let Some(process) = table.processes.get_mut(&process_id) else {
            return Err(SignalError::NoSuchProcess);
        };

        if !matches!(process.state, ProcessState::Running) {
            return Err(SignalError::NoSuchProcess);
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

    process_signal(signal);
}

fn take_latest_signal() -> Option<Signal> {
    let mut table = PROCESS_TABLE.lock();
    let process = table.current_process()?;

    process.take_latest_signal()
}

fn process_signal(signal: Signal) -> ! {
    match signal.default_action() {
        SignalAction::Terminate => crate::exit_current(ExitStatus::signaled(signal)),
    }
}

impl Process {
    fn queue_signal(&mut self, signal: Signal) {
        self.pending_signals.push(signal);
    }

    fn take_latest_signal(&mut self) -> Option<Signal> {
        self.pending_signals.pop()
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_memory::statistics;
    use roxy_signal::Signal;
    use roxy_test::kernel_test;
    use roxy_thread::Thread;
    use roxy_vm::AddrSpace;

    use super::Process;

    kernel_test!(
        "roxy-process::pending-signals-keep-order",
        pending_signals_keep_order,
        {
            let baseline = statistics().allocated_frames;
            let address_space = AddrSpace::new().unwrap().into_handle();
            let thread = Thread::new(unused_thread).unwrap();
            let mut process =
                Process::new(thread.id(), address_space.clone(), roxy_fd::FdTable::new());

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

    fn unused_thread() -> ! {
        panic!("unused process test thread started")
    }
}
