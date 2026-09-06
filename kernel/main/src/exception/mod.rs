use roxy_arch::{Architecture, CurrentArchitectureBackend, ExceptionContext, ExceptionVector};
use roxy_process::{ExitStatus, SignalAction, exit_current, signal_action_of};
use roxy_serial::e_println;
use roxy_signal::Signal;

/// Handles a CPU exception.
///
/// Faults raised while executing in user mode are converted into the corresponding signal and
/// terminate the faulting process, so its parent observes a normal `Signaled` exit. Exceptions
/// raised in kernel mode, and vectors that cannot be recovered by terminating a single process,
/// still panic the kernel with the original diagnostic.
pub(crate) fn handler(context: &ExceptionContext) -> ! {
    match context.vector {
        // A stop-NMI broadcast by a peer CPU during a system panic/halt. We must stop immediately
        // without touching the console (don't contend the serial lock with the panicking core) or
        // going through the panic path (which would re-broadcast and cascade NMI storms). Only the
        // initiating core confirms the shutdown; peers just halt.
        ExceptionVector::NonMaskable => CurrentArchitectureBackend::halt_forever(),
        _ => match (is_user_mode(context), fault_signal(context.vector)) {
            (true, Some(signal)) => terminate_for_fault(signal),
            _ => report_and_halt(context),
        },
    }
}

/// Whether the exception interrupted user-mode code (CS selector RPL == 3).
fn is_user_mode(context: &ExceptionContext) -> bool {
    context.code_segment & 3 == 3
}

/// Maps a fault vector to the signal a user process should receive, or `None` for vectors that
/// must not be recovered by terminating the faulting process (e.g. `DoubleFault`).
fn fault_signal(vector: ExceptionVector) -> Option<Signal> {
    match vector {
        ExceptionVector::PageFault | ExceptionVector::GeneralProtectionFault => {
            Some(Signal::SegmentationFault)
        }
        ExceptionVector::DivideError => Some(Signal::FloatingPointException),
        ExceptionVector::InvalidOpcode => Some(Signal::IllegalInstruction),
        // NonMaskable is handled before `fault_signal` is consulted (a stop-NMI always halts);
        // the `None` keeps this match exhaustive.
        ExceptionVector::DoubleFault | ExceptionVector::NonMaskable => None,
    }
}

/// Terminates the faulting process reporting that it was killed by `signal`.
///
/// The exception entry path does not preserve the full user register set, so a user-installed
/// handler cannot be delivered from here; any non-default disposition is reported as unsupported
/// before terminating, mirroring Linux `force_sig` which refuses to let the process continue
/// after a fault.
fn terminate_for_fault(signal: Signal) -> ! {
    if !matches!(signal_action_of(signal), SignalAction::Default) {
        e_println!(
            "UNSUPPORTED exception.delivery signal={} disposition",
            signal.number()
        );
    }

    exit_current(ExitStatus::signaled(signal))
}

fn report_and_halt(context: &ExceptionContext) -> ! {
    e_println!(
        "Exception: {:?} vector_error={:?} rip={:#x} rsp={:#x} cs={:#x} ss={:#x} rflags={:#x} cr2={:?} cpu={}",
        context.vector,
        context.error_code,
        context.instruction_pointer,
        context.stack_pointer,
        context.code_segment,
        context.stack_segment,
        context.cpu_flags,
        context.fault_address,
        context.cpu_id
    );
    crate::halt_all_cpus()
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_arch::ExceptionVector;
    use roxy_signal::Signal;
    use roxy_test::kernel_test;

    use super::{fault_signal, is_user_mode};
    use roxy_arch::CpuId;
    use roxy_arch::ExceptionContext;

    kernel_test!("roxy-main::exception-fault-signal-map", fault_signal_map, {
        assert_eq!(
            fault_signal(ExceptionVector::PageFault),
            Some(Signal::SegmentationFault)
        );
        assert_eq!(
            fault_signal(ExceptionVector::GeneralProtectionFault),
            Some(Signal::SegmentationFault)
        );
        assert_eq!(
            fault_signal(ExceptionVector::DivideError),
            Some(Signal::FloatingPointException)
        );
        assert_eq!(
            fault_signal(ExceptionVector::InvalidOpcode),
            Some(Signal::IllegalInstruction)
        );
        assert_eq!(fault_signal(ExceptionVector::DoubleFault), None);
    });

    kernel_test!(
        "roxy-main::exception-user-mode-detection",
        user_mode_detection,
        {
            fn context(code_segment: u64) -> ExceptionContext {
                ExceptionContext {
                    vector: ExceptionVector::PageFault,
                    error_code: Some(0),
                    instruction_pointer: 0x41_0000,
                    stack_pointer: 0x7fff_ffff_f000,
                    code_segment,
                    stack_segment: 0x2b,
                    cpu_flags: 0x202,
                    fault_address: Some(0),
                    cpu_id: CpuId::BSP,
                }
            }

            // User code segment selectors have RPL 3 (0x1b, 0x33, ...).
            assert!(is_user_mode(&context(0x1b)));
            assert!(is_user_mode(&context(0x33)));
            // Kernel selectors have RPL 0.
            assert!(!is_user_mode(&context(0x8)));
            assert!(!is_user_mode(&context(0x10)));
        }
    );
}
