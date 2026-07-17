use core::{arch::naked_asm, mem::size_of};

use super::ContextBackend;
use crate::stack::KernelStack;

#[repr(C)]
pub(super) struct X86_64Context {
    stack_pointer: usize,
}

/// Stack layout consumed by `switch_context` when a thread runs for the first time.
///
/// `switch_context` treats this frame exactly like a context saved by its push sequence: it pops
/// `r15` through `rbx`, leaving `instruction_pointer` at the top of the stack, then `ret` enters
/// `thread_start`. The frame places the real thread entry in `r12`, so `thread_start` can enable
/// interrupts and jump to it without needing another Rust call frame. `return_address` remains on
/// the stack as a trap in case an entry declared as `fn() -> !` unexpectedly returns.
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
    fn new(stack: &KernelStack, entry: fn() -> !) -> Self {
        let frame_pointer = stack
            .top_address()
            .checked_sub(size_of::<InitialStackFrame>())
            .unwrap() as *mut InitialStackFrame;
        let frame = InitialStackFrame {
            r15: 0,
            r14: 0,
            r13: 0,
            r12: entry as *const () as usize,
            rbp: 0,
            rbx: 0,
            instruction_pointer: thread_start as *const () as usize,
            return_address: thread_returned as *const () as usize,
        };
        // SAFETY: KernelStack owns one aligned, writable frame below its top address.
        unsafe {
            frame_pointer.write(frame);
        }
        Self {
            stack_pointer: frame_pointer as usize,
        }
    }

    fn empty() -> Self {
        Self { stack_pointer: 0 }
    }

    unsafe fn switch(previous: *mut Self, next: *const Self) {
        // SAFETY: The caller guarantees valid exclusive contexts and live backing stacks.
        unsafe { switch_context(previous, next) };
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
extern "C" fn thread_start() -> ! {
    // switch_context restored the thread entry into r12 before returning here. STI takes effect
    // after the following jump, so the entry begins with timer interrupts enabled.
    naked_asm!("sti", "jmp r12");
}

fn thread_returned() -> ! {
    panic!("thread entry returned")
}
