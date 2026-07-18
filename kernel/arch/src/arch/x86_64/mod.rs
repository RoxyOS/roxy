mod exception;
mod init;
mod interrupt;
mod syscall;
mod user;

use crate::{CpuId, ExceptionHandler, LocalInterruptHandler, LocalInterruptKind, SyscallHandler};

use super::{Architecture, sealed};

pub struct X86_64;

impl sealed::Sealed for X86_64 {}

impl Architecture for X86_64 {
    fn initialize(
        exception_handler: ExceptionHandler,
        local_interrupt_handler: LocalInterruptHandler,
    ) {
        init::initialize(exception_handler, local_interrupt_handler);
    }

    fn local_interrupt_vector(kind: LocalInterruptKind) -> u8 {
        interrupt::vector(kind)
    }

    fn current_cpu_id() -> CpuId {
        CpuId::BSP
    }

    fn interrupts_enabled() -> bool {
        ::x86_64::instructions::interrupts::are_enabled()
    }

    fn without_interrupts<T>(function: impl FnOnce() -> T) -> T {
        ::x86_64::instructions::interrupts::without_interrupts(function)
    }

    fn enable_interrupts() {
        ::x86_64::instructions::interrupts::enable();
    }

    unsafe fn enter_user(
        user_instruction_pointer: u64,
        user_stack_pointer: u64,
        kernel_stack_top: u64,
    ) -> ! {
        // SAFETY: The caller guarantees valid user mappings and the backend supplies valid selectors.
        unsafe {
            user::enter(
                user_instruction_pointer,
                user_stack_pointer,
                kernel_stack_top,
            )
        }
    }

    fn configure_syscall(handler: SyscallHandler) {
        syscall::configure(handler);
    }

    fn set_kernel_stack_top(kernel_stack_top: u64) {
        syscall::set_kernel_stack_top(kernel_stack_top);
    }

    fn wait_for_interrupt() {
        interrupt::wait();
    }

    fn halt() {
        ::x86_64::instructions::hlt();
    }

    fn halt_forever() -> ! {
        ::x86_64::instructions::interrupts::disable();

        loop {
            Self::halt();
        }
    }
}
