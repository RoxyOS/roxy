use core::{
    arch::naked_asm,
    sync::atomic::{AtomicU64, Ordering},
};

use roxy_arch::{Architecture, CurrentArchitectureBackend};
use roxy_memory::VirtualAddress;

const EXIT_SYSCALL: u64 = 0;

static KERNEL_STACK_TOP: AtomicU64 = AtomicU64::new(0);

pub(super) unsafe fn initialize(kernel_stack_top: VirtualAddress) {
    assert_eq!(
        KERNEL_STACK_TOP.swap(kernel_stack_top.as_u64(), Ordering::AcqRel),
        0,
        "syscall initialized twice"
    );
    // SAFETY: syscall_entry is permanent and obeys the x86_64 syscall entry contract.
    unsafe {
        CurrentArchitectureBackend::configure_syscall(syscall_entry as *const () as u64);
    }
}

#[unsafe(naked)]
unsafe extern "C" fn syscall_entry() -> ! {
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

extern "C" fn dispatch(number: u64, status: u64) -> ! {
    assert_eq!(number, EXIT_SYSCALL, "unknown syscall {number}");
    // TODO: Store status in the current Process when process execution is connected.
    let _ = status;
    roxy_thread::scheduler::exit_current()
}
