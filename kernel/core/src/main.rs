#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

mod exception;
mod initial_fds;
mod interrupt;
mod misc;
mod rootfs;
#[cfg(feature = "kernel-test")]
mod test;

use core::{alloc::Layout, panic::PanicInfo};

use roxy_arch::{Architecture, CurrentArchitectureBackend};
use roxy_boot::BootInfo;
use roxy_serial::e_println;

#[cfg(not(feature = "kernel-test"))]
const INIT: &[u8] = b"/usr/bin/bash";

#[unsafe(no_mangle)]
#[allow(clippy::missing_panics_doc)]
pub extern "C" fn _start() -> ! {
    misc::clear_bss();
    roxy_serial::initialize();

    let boot_info = BootInfo::parse();

    CurrentArchitectureBackend::initialize(exception::handler, interrupt::handler);
    roxy_memory::initialize(&boot_info);
    roxy_time::initialize(boot_info.unix_seconds_at_boot);
    rootfs::initialize(&boot_info).expect("initialize root filesystem");
    roxy_cpu::current_cpu().initialize();
    roxy_process::initialize(initial_fds::inject);
    roxy_futex::initialize();
    roxy_syscall::initialize();
    CurrentArchitectureBackend::enable_interrupts();

    #[cfg(feature = "kernel-test")]
    test::run();

    #[cfg(not(feature = "kernel-test"))]
    kernel_main()
}

#[cfg(not(feature = "kernel-test"))]
fn kernel_main() -> ! {
    roxy_process::spawn(INIT).expect("spawn init process");

    roxy_thread::scheduler::start()
}

#[panic_handler]
fn panic(info: &PanicInfo<'_>) -> ! {
    e_println!("Kernel Panic: {info}");

    #[cfg(feature = "kernel-test")]
    test::exit_failure();

    #[cfg(not(feature = "kernel-test"))]
    CurrentArchitectureBackend::halt_forever()
}

#[alloc_error_handler]
fn allocation_error(layout: Layout) -> ! {
    let stats = roxy_memory::statistics();
    let cpu = CurrentArchitectureBackend::current_cpu_id();

    e_println!(
        "Kernel heap OOM: size={}, align={}, cpu={}, stats={stats:?}, process/thread=unavailable",
        layout.size(),
        layout.align(),
        cpu,
    );
    panic!("kernel heap exhausted")
}
