use core::{
    arch::naked_asm,
    mem::{offset_of, size_of},
    sync::atomic::{AtomicU64, AtomicUsize, Ordering},
};

use x86_64::{
    VirtAddr,
    registers::{
        model_specific::{Efer, EferFlags, LStar, SFMask, Star},
        rflags::RFlags,
    },
};

use crate::{RawSyscall, SyscallHandler};

use super::init;

static KERNEL_STACK_TOP: AtomicU64 = AtomicU64::new(0);
static HANDLER: AtomicUsize = AtomicUsize::new(0);

#[repr(C)]
struct EntryFrame {
    request: RawSyscall,
    user_instruction_pointer: u64,
    user_flags: u64,
    user_stack_pointer: u64,
}

const _: () = {
    assert!(offset_of!(EntryFrame, request) == 0);
    assert!(offset_of!(EntryFrame, user_instruction_pointer) == 56);
    assert!(offset_of!(EntryFrame, user_flags) == 64);
    assert!(offset_of!(EntryFrame, user_stack_pointer) == 72);
    assert!(size_of::<EntryFrame>() == 80);
};

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
        "xchg rsp, [rip + {kernel_stack_top}]",
        "push qword ptr [rip + {kernel_stack_top}]",
        "push r11",
        "push rcx",
        "push r9",
        "push r8",
        "push r10",
        "push rdx",
        "push rsi",
        "push rdi",
        "push rax",
        "mov rdi, rsp",
        "call {dispatch}",
        "mov rcx, [rsp + {user_instruction_pointer}]",
        "mov r11, [rsp + {user_flags}]",
        "lea rdx, [rsp + {frame_size}]",
        "mov [rip + {kernel_stack_top}], rdx",
        "mov rsp, [rsp + {user_stack_pointer}]",
        "sysretq",
        kernel_stack_top = sym KERNEL_STACK_TOP,
        dispatch = sym dispatch,
        user_instruction_pointer = const offset_of!(EntryFrame, user_instruction_pointer),
        user_flags = const offset_of!(EntryFrame, user_flags),
        user_stack_pointer = const offset_of!(EntryFrame, user_stack_pointer),
        frame_size = const size_of::<EntryFrame>(),
    )
}

extern "C" fn dispatch(frame: *const EntryFrame) -> u64 {
    let address = HANDLER.load(Ordering::Acquire);
    assert_ne!(address, 0, "syscall handler not initialized");

    // SAFETY: entry passes a pointer to its complete, live EntryFrame on the kernel stack.
    let request = unsafe { (*frame).request };
    // SAFETY: configure stores one permanent SyscallHandler function pointer.
    let handler: SyscallHandler = unsafe { core::mem::transmute(address) };
    handler(request)
}
