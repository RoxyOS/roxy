use core::{
    arch::naked_asm,
    mem::{offset_of, size_of},
};

use roxy_arch::{Architecture, CurrentArchitectureBackend};
use roxy_memory::UserAddress;

use super::ContextBackend;
use crate::stack::KernelStack;

#[repr(C)]
pub(super) struct X86_64Context {
    stack_pointer: usize,
    fs_base: u64,
}

const _: () = assert!(offset_of!(X86_64Context, stack_pointer) == 0);

/// Stack layout consumed by `switch_context` when a thread runs for the first time.
///
/// `switch_context` treats this frame exactly like a context saved by its push sequence: it pops
/// `r15` through `rbx`, leaving `instruction_pointer` at the top of the stack. Kernel threads place
/// their entry in `r12`; user threads place user RIP, user RSP, and kernel-stack top in `r12-r14`.
/// The selected trampoline consumes those values after `ret`. `return_address` remains on the
/// stack as a trap in case a non-returning trampoline unexpectedly returns.
#[repr(C)]
struct InitialStackFrame {
    r15: usize,
    r14: usize,
    r13: usize,
    r12: usize,
    rbp: usize,
    rbx: usize,
    instruction_pointer: usize,
    return_address: usize,
}

impl ContextBackend for X86_64Context {
    fn new_kernel(kernel_stack: &KernelStack, entry: fn() -> !) -> Self {
        Self::with_frame(
            kernel_stack,
            InitialStackFrame {
                r15: 0,
                r14: 0,
                r13: 0,
                r12: entry as *const () as usize,
                rbp: 0,
                rbx: 0,
                instruction_pointer: kernel_thread_start as *const () as usize,
                return_address: thread_returned as *const () as usize,
            },
        )
    }

    fn new_user(
        kernel_stack: &KernelStack,
        user_instruction_pointer: UserAddress,
        user_stack_pointer: UserAddress,
    ) -> Self {
        Self::with_frame(
            kernel_stack,
            InitialStackFrame {
                r15: 0,
                r14: kernel_stack.top_address(),
                r13: usize::try_from(user_stack_pointer.as_u64()).unwrap(),
                r12: usize::try_from(user_instruction_pointer.as_u64()).unwrap(),
                rbp: 0,
                rbx: 0,
                instruction_pointer: user_thread_start as *const () as usize,
                return_address: thread_returned as *const () as usize,
            },
        )
    }

    fn empty() -> Self {
        Self {
            stack_pointer: 0,
            fs_base: 0,
        }
    }

    unsafe fn switch(previous: *mut Self, next: *const Self) {
        // SAFETY: the caller guarantees distinct, exclusively owned live contexts.
        let (previous, next) = unsafe { (&mut *previous, &*next) };
        previous.fs_base = CurrentArchitectureBackend::user_thread_pointer();
        CurrentArchitectureBackend::set_user_thread_pointer(next.fs_base);

        // SAFETY: The caller guarantees valid exclusive contexts and live backing stacks.
        unsafe { switch_context(previous, next) };
    }
}

impl X86_64Context {
    fn with_frame(kernel_stack: &KernelStack, frame: InitialStackFrame) -> Self {
        let frame_pointer = kernel_stack
            .top_address()
            .checked_sub(size_of::<InitialStackFrame>())
            .unwrap() as *mut InitialStackFrame;
        // SAFETY: KernelStack owns one aligned, writable frame below its top address.
        unsafe {
            frame_pointer.write(frame);
        }
        Self {
            stack_pointer: frame_pointer as usize,
            fs_base: 0,
        }
    }
}

#[unsafe(naked)]
unsafe extern "C" fn switch_context(_previous: *mut X86_64Context, _next: *const X86_64Context) {
    naked_asm!(
        "push rbx",
        "push rbp",
        "push r12",
        "push r13",
        "push r14",
        "push r15",
        "mov [rdi], rsp",
        "mov rsp, [rsi]",
        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop rbp",
        "pop rbx",
        "ret",
    );
}

#[unsafe(naked)]
extern "C" fn kernel_thread_start() -> ! {
    // switch_context restored the thread entry into r12 before returning here. STI takes effect
    // after the following jump, so the entry begins with timer interrupts enabled.
    naked_asm!("sti", "jmp r12");
}

#[unsafe(naked)]
extern "C" fn user_thread_start() -> ! {
    // switch_context restores user RIP, user RSP, and kernel-stack top into r12-r14. This
    // trampoline moves them into ABI argument registers while interrupts remain disabled.
    naked_asm!(
        "mov rdi, r12",
        "mov rsi, r13",
        "mov rdx, r14",
        "jmp {enter_user}",
        enter_user = sym enter_user,
    );
}

extern "C" fn enter_user(
    user_instruction_pointer: u64,
    user_stack_pointer: u64,
    kernel_stack_top: u64,
) -> ! {
    // SAFETY: construction accepts typed user addresses and owns the live kernel stack.
    unsafe {
        CurrentArchitectureBackend::enter_user(
            user_instruction_pointer,
            user_stack_pointer,
            kernel_stack_top,
        )
    }
}

fn thread_returned() -> ! {
    panic!("thread entry returned")
}
