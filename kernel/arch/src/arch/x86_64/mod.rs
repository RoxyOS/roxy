mod cpu_map;
mod exception;
mod float;
mod init;
mod interrupt;
mod syscall;
mod user;

pub use float::X86_64FloatState;
pub use syscall::X86_64UserContext;

use ::x86_64::{VirtAddr, registers::model_specific::FsBase};

use crate::{CpuId, ExceptionHandler, Interrupt, InterruptDispatcher, SyscallHandler};

use super::{Architecture, sealed};

pub struct X86_64;

impl sealed::Sealed for X86_64 {}

impl Architecture for X86_64 {
    fn initialize(exception_handler: ExceptionHandler) {
        // The BSP registers itself before any other code can query `current_cpu_id`, so slot 0
        // is claimed before `Lock`/`CpuLocal` first index per-CPU storage during memory setup.
        cpu_map::register(cpu_map::read_current_apic_id());
        assert_eq!(
            Self::current_cpu_id(),
            CpuId::BSP,
            "BSP must claim logical slot 0"
        );
        init::initialize(exception_handler);
    }

    fn register_interrupt_dispatcher(dispatcher: InterruptDispatcher) {
        interrupt::register(dispatcher);
    }

    fn interrupt_vector(interrupt: Interrupt) -> u8 {
        interrupt::vector(interrupt)
    }

    fn current_cpu_id() -> CpuId {
        cpu_map::current_id()
    }

    fn current_stack_pointer() -> u64 {
        let rsp: u64;
        // SAFETY: Reading RSP is always safe and has no side effects.
        unsafe { core::arch::asm!("mov {}, rsp", out(reg) rsp, options(nostack, preserves_flags)) };
        rsp
    }

    fn initialize_application_processor(kernel_stack_top: u64) {
        init::initialize_ap(kernel_stack_top);
    }

    fn register_application_processor() {
        init::register_ap();
    }

    fn ap_kernel_stack_top(cpu_id: CpuId) -> u64 {
        init::kernel_stack_top(cpu_id)
    }

    unsafe fn switch_stack_pt_and_call(
        stack_top: u64,
        page_table_root_phys: u64,
        continuation: extern "C" fn() -> !,
    ) -> ! {
        // SAFETY: the caller guarantees the stack/page-table contract documented on the trait.
        unsafe { init::switch_stack_pt_and_call(stack_top, page_table_root_phys, continuation) }
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

    unsafe fn resume_user(instruction_pointer: u64, stack_pointer: u64) -> ! {
        // SAFETY: the caller guarantees that both addresses are valid in the active user image.
        unsafe { syscall::resume_user(instruction_pointer, stack_pointer) }
    }

    fn set_kernel_stack_top(kernel_stack_top: u64) {
        user::set_kernel_stack_top(kernel_stack_top);
        syscall::set_kernel_stack_top(kernel_stack_top);
    }

    fn user_thread_pointer() -> u64 {
        FsBase::read().as_u64()
    }

    fn set_user_thread_pointer(pointer: u64) {
        FsBase::write(VirtAddr::new(pointer));
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

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_test::kernel_test;

    use super::{Architecture, X86_64, syscall, user};

    const TEST_STACK_TOP: u64 = 0xffff_8000_0000_1000;

    kernel_test!(
        "roxy-arch::privileged-entry-stack",
        privileged_entry_stack,
        {
            X86_64::without_interrupts(|| {
                let previous_tss = user::kernel_stack_top();
                let previous_syscall = syscall::kernel_stack_top();

                X86_64::set_kernel_stack_top(TEST_STACK_TOP);
                assert_eq!(user::kernel_stack_top(), TEST_STACK_TOP);
                assert_eq!(syscall::kernel_stack_top(), TEST_STACK_TOP);

                user::set_kernel_stack_top(previous_tss);
                syscall::set_kernel_stack_top(previous_syscall);
            });
        }
    );
}
