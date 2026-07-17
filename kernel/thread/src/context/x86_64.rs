use core::{arch::naked_asm, mem::size_of};

use super::ContextBackend;
use crate::stack::KernelStack;

const SAVED_WORDS: usize = 8;
const ENTRY_WORD: usize = 6;
const RETURN_WORD: usize = 7;

#[repr(C)]
pub(super) struct X86_64Context {
    stack_pointer: usize,
}

impl ContextBackend for X86_64Context {
    fn new(stack: &KernelStack, entry: fn() -> !) -> Self {
        let stack_pointer = stack
            .top_address()
            .checked_sub(SAVED_WORDS * size_of::<usize>())
            .unwrap() as *mut usize;
        // SAFETY: KernelStack owns eight writable words below its aligned top address.
        unsafe {
            stack_pointer.write_bytes(0, SAVED_WORDS);
            stack_pointer
                .add(ENTRY_WORD)
                .write(entry as *const () as usize);
            stack_pointer
                .add(RETURN_WORD)
                .write(thread_returned as *const () as usize);
        }
        Self {
            stack_pointer: stack_pointer as usize,
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

fn thread_returned() -> ! {
    panic!("thread entry returned")
}
