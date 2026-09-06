use core::arch::naked_asm;

use x86_64::VirtAddr;

use super::{init, syscall};

pub(super) unsafe fn enter(
    user_instruction_pointer: u64,
    user_stack_pointer: u64,
    kernel_stack_top: u64,
) -> ! {
    assert!(!x86_64::instructions::interrupts::are_enabled());

    set_kernel_stack_top(kernel_stack_top);

    let (code_selector, data_selector) = init::user_selectors();
    // SAFETY: initialization installed these selectors and the caller validated both user mappings.
    unsafe {
        iret_to_user(
            user_instruction_pointer,
            user_stack_pointer,
            code_selector,
            data_selector,
        )
    }
}

pub(super) fn set_kernel_stack_top(kernel_stack_top: u64) {
    assert!(!x86_64::instructions::interrupts::are_enabled());

    let tss = syscall::current_cpu_tss();

    // SAFETY: interrupts are disabled on this CPU, so its per-CPU TSS is owned exclusively here.
    unsafe { (*tss).privilege_stack_table[0] = VirtAddr::new(kernel_stack_top) };
}

#[cfg(feature = "kernel-test")]
pub(super) fn kernel_stack_top() -> u64 {
    let tss = syscall::current_cpu_tss();

    // SAFETY: tests read the current CPU's TSS without concurrent mutation.
    unsafe { (*tss).privilege_stack_table[0].as_u64() }
}

#[unsafe(naked)]
unsafe extern "C" fn iret_to_user(
    _user_instruction_pointer: u64,
    _user_stack_pointer: u64,
    _code_selector: u64,
    _data_selector: u64,
) -> ! {
    naked_asm!(
        "mov ax, cx",
        "mov ds, ax",
        "mov es, ax",
        "push rcx",
        "push rsi",
        "push 0x202",
        "push rdx",
        "push rdi",
        "iretq",
    )
}
