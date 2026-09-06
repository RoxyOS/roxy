use core::{
    arch::naked_asm,
    mem::{offset_of, size_of},
    sync::atomic::AtomicBool,
};

use roxy_arch::{Architecture, CurrentArchitectureBackend, FloatState, UserContext};
use roxy_memory::UserAddress;

use super::ContextBackend;
use crate::stack::KernelStack;

#[repr(C)]
pub(super) struct X86_64Context {
    stack_pointer: usize,
    fs_base: u64,
    float_state: FloatState,
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

    fn new_user_resume(kernel_stack: &KernelStack, context: UserContext) -> Self {
        let fs_base = context.fs_base;
        let context_pointer = (kernel_stack.top_address()
            - size_of::<InitialStackFrame>()
            - size_of::<UserContext>()) as *mut UserContext;
        // SAFETY: the context is stored below the initial frame in this thread-owned stack.
        unsafe { context_pointer.write(context) };

        let mut saved_context = Self::with_frame(
            kernel_stack,
            InitialStackFrame {
                r15: 0,
                r14: 0,
                r13: 0,
                r12: context_pointer as usize,
                rbp: 0,
                rbx: 0,
                instruction_pointer: user_resume_start as *const () as usize,
                return_address: thread_returned as *const () as usize,
            },
        );
        saved_context.fs_base = fs_base;
        // SAFETY: Architecture initialization configures FXSAVE before any thread is created.
        saved_context.float_state = unsafe { FloatState::capture_current() };
        saved_context
    }

    fn empty() -> Self {
        Self {
            stack_pointer: 0,
            fs_base: 0,
            float_state: FloatState::initial(),
        }
    }

    unsafe fn switch(previous: *mut Self, next: *const Self, reserved_ptr: *const AtomicBool) {
        // SAFETY: Both contexts are reserved for this CPU until the assembly handoff. Field
        // borrows end before that handoff; no Rust reference spans the suspension or later reap.
        unsafe {
            (*previous).fs_base = CurrentArchitectureBackend::user_thread_pointer();
            CurrentArchitectureBackend::set_user_thread_pointer((*next).fs_base);
            (*previous).float_state.save();
            (*next).float_state.restore();
            switch_context(previous, next, reserved_ptr);
        }
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
            float_state: FloatState::initial(),
        }
    }
}

// Internal SysV x86_64 calling convention: RDI/RSI point to contexts, RDX to an optional
// AtomicBool. Its one-byte store is a release on x86_64 (ordered stores), paired with Acquire
// in dispatch/reap. After changing RSP and clearing the flag, never access outgoing memory.
const _: () = assert!(size_of::<AtomicBool>() == 1);

#[unsafe(naked)]
unsafe extern "C" fn switch_context(
    _previous: *mut X86_64Context,
    _next: *const X86_64Context,
    _reserved_ptr: *const AtomicBool,
) {
    naked_asm!(
        "push rbx",
        "push rbp",
        "push r12",
        "push r13",
        "push r14",
        "push r15",
        "mov [rdi], rsp",
        "mov rsp, [rsi]",
        "test rdx, rdx",
        "jz 2f",
        "mov byte ptr [rdx], 0",
        "2:",
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

#[unsafe(naked)]
unsafe extern "C" fn user_resume_start() -> ! {
    naked_asm!(
        "mov r15, [r12 + {r15}]",
        "mov r14, [r12 + {r14}]",
        "mov r13, [r12 + {r13}]",
        "mov rbp, [r12 + {rbp}]",
        "mov rbx, [r12 + {rbx}]",
        "mov rax, [r12 + {rax}]",
        "mov rdi, [r12 + {rdi}]",
        "mov rsi, [r12 + {rsi}]",
        "mov rdx, [r12 + {rdx}]",
        "mov r10, [r12 + {r10}]",
        "mov r8, [r12 + {r8}]",
        "mov r9, [r12 + {r9}]",
        "mov rcx, [r12 + {instruction_pointer}]",
        "mov r11, [r12 + {flags}]",
        "mov rsp, [r12 + {stack_pointer}]",
        "mov r12, [r12 + {r12}]",
        "sysretq",
        r15 = const offset_of!(UserContext, r15),
        r14 = const offset_of!(UserContext, r14),
        r13 = const offset_of!(UserContext, r13),
        r12 = const offset_of!(UserContext, r12),
        rbp = const offset_of!(UserContext, rbp),
        rbx = const offset_of!(UserContext, rbx),
        rax = const offset_of!(UserContext, rax),
        rdi = const offset_of!(UserContext, rdi),
        rsi = const offset_of!(UserContext, rsi),
        rdx = const offset_of!(UserContext, rdx),
        r10 = const offset_of!(UserContext, r10),
        r8 = const offset_of!(UserContext, r8),
        r9 = const offset_of!(UserContext, r9),
        instruction_pointer = const offset_of!(UserContext, instruction_pointer),
        flags = const offset_of!(UserContext, flags),
        stack_pointer = const offset_of!(UserContext, stack_pointer),
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
