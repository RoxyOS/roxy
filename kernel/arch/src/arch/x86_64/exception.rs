use core::sync::atomic::{AtomicUsize, Ordering};

use x86_64::{
    registers::control::Cr2,
    structures::idt::{InterruptStackFrame, PageFaultErrorCode},
};

use crate::{
    Architecture, CurrentArchitectureBackend, ExceptionContext, ExceptionHandler, ExceptionVector,
    arch::X86_64,
};

static HANDLER: AtomicUsize = AtomicUsize::new(0);

pub(super) fn register(handler: ExceptionHandler) {
    HANDLER.store(handler as usize, Ordering::Release);
}

fn dispatch(
    vector: ExceptionVector,
    frame: &InterruptStackFrame,
    error_code: Option<u64>,
    fault_address: Option<u64>,
) -> ! {
    let context = ExceptionContext {
        vector,
        error_code,
        instruction_pointer: frame.instruction_pointer.as_u64(),
        stack_pointer: frame.stack_pointer.as_u64(),
        code_segment: u64::from(frame.code_segment.0),
        stack_segment: u64::from(frame.stack_segment.0),
        cpu_flags: frame.cpu_flags.bits(),
        fault_address,
        cpu_id: CurrentArchitectureBackend::current_cpu_id(),
    };

    let address = HANDLER.load(Ordering::Acquire);

    if address == 0 {
        X86_64::halt_forever();
    }
    // SAFETY: register stores a valid ExceptionHandler function pointer once.
    let handler: ExceptionHandler = unsafe { core::mem::transmute(address) };
    handler(&context)
}

pub(super) extern "x86-interrupt" fn divide_error(frame: InterruptStackFrame) {
    dispatch(ExceptionVector::DivideError, &frame, None, None)
}

pub(super) extern "x86-interrupt" fn invalid_opcode(frame: InterruptStackFrame) {
    dispatch(ExceptionVector::InvalidOpcode, &frame, None, None)
}

pub(super) extern "x86-interrupt" fn general_protection_fault(
    frame: InterruptStackFrame,
    code: u64,
) {
    dispatch(
        ExceptionVector::GeneralProtectionFault,
        &frame,
        Some(code),
        None,
    )
}

pub(super) extern "x86-interrupt" fn page_fault(
    frame: InterruptStackFrame,
    code: PageFaultErrorCode,
) {
    dispatch(
        ExceptionVector::PageFault,
        &frame,
        Some(code.bits()),
        Some(Cr2::read_raw()),
    )
}

pub(super) extern "x86-interrupt" fn non_maskable_interrupt(frame: InterruptStackFrame) {
    dispatch(ExceptionVector::NonMaskable, &frame, None, None)
}

pub(super) extern "x86-interrupt" fn double_fault(frame: InterruptStackFrame, code: u64) -> ! {
    dispatch(ExceptionVector::DoubleFault, &frame, Some(code), None)
}
