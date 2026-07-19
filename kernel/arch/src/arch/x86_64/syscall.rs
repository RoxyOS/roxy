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

use crate::{Architecture, RawSyscall, SyscallHandler};

use super::init;

static KERNEL_STACK_TOP: AtomicU64 = AtomicU64::new(0);
static HANDLER: AtomicUsize = AtomicUsize::new(0);

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct X86_64UserContext {
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub rbp: u64,
    pub rbx: u64,
    pub rax: u64,
    pub rdi: u64,
    pub rsi: u64,
    pub rdx: u64,
    pub r10: u64,
    pub r8: u64,
    pub r9: u64,
    pub instruction_pointer: u64,
    pub flags: u64,
    pub stack_pointer: u64,
    pub fs_base: u64,
}

impl X86_64UserContext {
    #[must_use]
    pub const fn with_syscall_result(mut self, result: u64) -> Self {
        self.rax = result;
        self
    }
}

#[repr(C)]
struct EntryFrame {
    r15: u64,
    r14: u64,
    r13: u64,
    r12: u64,
    rbp: u64,
    rbx: u64,
    rax: u64,
    rdi: u64,
    rsi: u64,
    rdx: u64,
    r10: u64,
    r8: u64,
    r9: u64,
    user_instruction_pointer: u64,
    user_flags: u64,
    user_stack_pointer: u64,
}

const _: () = {
    assert!(offset_of!(EntryFrame, user_instruction_pointer) == 104);
    assert!(offset_of!(EntryFrame, user_flags) == 112);
    assert!(offset_of!(EntryFrame, user_stack_pointer) == 120);
    assert!(size_of::<EntryFrame>() == 128);
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
        "push rbx",
        "push rbp",
        "push r12",
        "push r13",
        "push r14",
        "push r15",
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
    let frame = unsafe { &*frame };
    let request = RawSyscall {
        number: frame.rax,
        arguments: [
            frame.rdi, frame.rsi, frame.rdx, frame.r10, frame.r8, frame.r9,
        ],
        context: X86_64UserContext {
            r15: frame.r15,
            r14: frame.r14,
            r13: frame.r13,
            r12: frame.r12,
            rbp: frame.rbp,
            rbx: frame.rbx,
            rax: frame.rax,
            rdi: frame.rdi,
            rsi: frame.rsi,
            rdx: frame.rdx,
            r10: frame.r10,
            r8: frame.r8,
            r9: frame.r9,
            instruction_pointer: frame.user_instruction_pointer,
            flags: frame.user_flags,
            stack_pointer: frame.user_stack_pointer,
            fs_base: super::super::CurrentArchitectureBackend::user_thread_pointer(),
        },
    };
    // SAFETY: configure stores one permanent SyscallHandler function pointer.
    let handler: SyscallHandler = unsafe { core::mem::transmute(address) };
    handler(request)
}
