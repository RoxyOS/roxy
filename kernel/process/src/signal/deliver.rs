//! Applying pending signals at a userspace-return boundary: handler frames, default actions,
//! and process stopping.

use roxy_arch::{ResumeInfo, SYSCALL_INSTRUCTION_SIZE, UserContext};
use roxy_signal::{DefaultAction, Signal, SignalSet};
use roxy_thread::scheduler;

use crate::{
    ExitStatus, ProcessState, exit_current,
    signal::{PendingSignal, SignalAction, signal_action_of},
    signal_frame,
    table::PROCESS_TABLE,
};

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

    // An `SA_SIGINFO` handler receives the record's address within its own frame; a plain handler
    // gets the signal number and zeroed arguments. The third argument is null: POSIX points it at
    // the interrupted machine context, and this ABI defines none, so there is no object to point
    // at.
    //
    // TODO(signal-handler-context): a handler therefore cannot inspect or redirect the context it
    // interrupted. Serving that means defining a context record here and exporting its layout,
    // which the ABI deliberately does not carry today.
    let arguments = if include_siginfo {
        [
            u64::from(signal.number()),
            frame_base + signal_frame::SIGINFO_OFFSET as u64,
            0,
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

    // The handler's `ret` pops the frame's leading return-address slot (the trampoline entry)
    // before the trampoline issues `sigreturn`, so the user stack pointer observed at syscall
    // entry is one slot above the recorded frame base. Refuse to restore a frame whose position
    // does not match that contract instead of trusting a foreign frame.
    if frame_address + signal_frame::RETURN_ADDRESS_SIZE as u64 != context.stack_pointer {
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

/// Pops the most recent unmasked pending signal for the current thread.
fn take_pending_unmasked_signal() -> Option<PendingSignal> {
    let mut table = PROCESS_TABLE.lock();
    let thread_id = scheduler::current_thread_id();
    let process = table.current_process()?;

    process.take_pending_for(thread_id)
}

fn do_default_action(signal: Signal, action: DefaultAction) {
    match action {
        DefaultAction::Terminate => exit_current(ExitStatus::signaled(signal)),
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
