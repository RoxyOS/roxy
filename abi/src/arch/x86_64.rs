use core::arch::asm;
#[cfg(all(feature = "userspace", not(test)))]
use core::panic::PanicInfo;

use super::{ArchitectureBackend, sealed};

pub(crate) struct X86_64ArchitectureBackend;

impl sealed::Sealed for X86_64ArchitectureBackend {}

impl ArchitectureBackend for X86_64ArchitectureBackend {
    unsafe fn syscall1_noreturn(number: u64, argument: u64) -> ! {
        // SAFETY: The caller guarantees the syscall number and argument follow the documented ABI.
        unsafe {
            asm!(
                "syscall",
                "ud2",
                in("rax") number,
                in("rdi") argument,
                options(noreturn),
            )
        }
    }
}

#[cfg(all(feature = "userspace", not(test)))]
#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    // SAFETY: The userspace ABI static library cannot unwind or report a Rust panic. Its wrappers
    // contain no expected panic path, so an unexpected panic terminates with an invalid opcode.
    unsafe { asm!("ud2", options(noreturn)) }
}
