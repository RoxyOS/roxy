use core::{
    arch::naked_asm,
    sync::atomic::{AtomicU64, AtomicUsize, Ordering},
};

use x86_64::{
    VirtAddr,
    registers::{
        model_specific::{Efer, EferFlags, LStar, SFMask, Star},
        rflags::RFlags,
    },
};

use crate::SyscallHandler;

use super::init;

static KERNEL_STACK_TOP: AtomicU64 = AtomicU64::new(0);
static HANDLER: AtomicUsize = AtomicUsize::new(0);

pub(super) fn configure(handler: SyscallHandler) {
    assert_eq!(
        HANDLER.swap(handler as usize, Ordering::AcqRel),
        0,
        "syscall initialized twice"
    );
    let (user_code, user_data, kernel_code, kernel_data) = init::syscall_selectors();
    // SAFETY: architecture initialization established long mode and a permanent entry point.
    unsafe { Efer::update(|flags| flags.insert(EferFlags::SYSTEM_CALL_EXTENSIONS)) };

    Star::write(user_code, user_data, kernel_code, kernel_data)
        .expect("invalid syscall segment layout");
    LStar::write(VirtAddr::new(entry as *const () as u64));
    SFMask::write(RFlags::INTERRUPT_FLAG);
}

pub(super) fn set_kernel_stack_top(kernel_stack_top: u64) {
    KERNEL_STACK_TOP.store(kernel_stack_top, Ordering::Release);
}

#[unsafe(naked)]
unsafe extern "C" fn entry() -> ! {
    naked_asm!(
        "mov rsp, [rip + {kernel_stack_top}]",
        "mov rsi, rdi",
        "mov rdi, rax",
        "call {dispatch}",
        "ud2",
        kernel_stack_top = sym KERNEL_STACK_TOP,
        dispatch = sym dispatch,
    )
}

extern "C" fn dispatch(number: u64, argument: u64) -> ! {
    let address = HANDLER.load(Ordering::Acquire);
    assert_ne!(address, 0, "syscall handler not initialized");

    // SAFETY: configure stores one permanent SyscallHandler function pointer.
    let handler: SyscallHandler = unsafe { core::mem::transmute(address) };
    handler(number, argument)
}
