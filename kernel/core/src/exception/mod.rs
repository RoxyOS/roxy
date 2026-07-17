use roxy_arch::{Architecture, CurrentArchitectureBackend, ExceptionContext};

use crate::e_println;

pub(crate) fn handler(context: &ExceptionContext) -> ! {
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
    CurrentArchitectureBackend::halt_forever()
}
