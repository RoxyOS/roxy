use qemu_exit::{QEMUExit, X86};

use crate::s_println;

const SUCCESS_STATUS: u32 = 33;
const EXIT: X86 = {
    // SAFETY: xtask configures one isa-debug-exit device at this exact I/O port.
    unsafe { X86::new(0xf4, SUCCESS_STATUS) }
};

pub(crate) fn run() -> ! {
    s_println!("==> Running {} kernel tests", roxy_test::TESTS.len());

    for test in roxy_test::TESTS {
        s_println!("[ RUN      ] {}", test.name);
        (test.run)();
        s_println!("[       OK ] {}", test.name);
    }

    s_println!("==> Kernel tests passed: {}", roxy_test::TESTS.len());
    EXIT.exit_success()
}

pub(crate) fn exit_failure() -> ! {
    EXIT.exit_failure()
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_arch::CpuId;

    roxy_test::kernel_test!(
        "roxy-kernel::subsystems-are-initialized",
        subsystems_are_initialized,
        {
            let cpu = roxy_cpu::current_cpu();
            let memory = roxy_memory::statistics();
            let addrspace = roxy_vm::AddrSpace::new().unwrap();
            let thread = roxy_thread::Thread::new(unused_thread).unwrap();
            let invalid_process = roxy_process::spawn(&[]);
            let _ = core::hint::black_box(roxy_syscall::initialize as fn());

            assert_eq!(cpu.id(), CpuId::BSP);
            assert!(memory.total_frames > 0);
            assert!(memory.heap_total_bytes > 0);
            assert!(memory.allocated_frames <= memory.total_frames);
            assert!(addrspace.root_address().as_u64() > 0);
            assert!(matches!(
                invalid_process,
                Err(roxy_process::ProcessError::InvalidElf)
            ));
            drop(thread);
        }
    );

    fn unused_thread() -> ! {
        panic!("unused thread was started")
    }
}
