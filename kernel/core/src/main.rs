#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

mod exception;
mod interrupt;
mod misc;
mod rootfs;
mod serial;
#[cfg(feature = "kernel-test")]
mod test;

use core::{alloc::Layout, panic::PanicInfo};

use roxy_arch::{Architecture, CurrentArchitectureBackend};
use roxy_boot::BootInfo;

#[unsafe(no_mangle)]
#[allow(clippy::missing_panics_doc)]
pub extern "C" fn _start() -> ! {
    misc::clear_bss();
    serial::initialize();

    let boot_info = BootInfo::parse();

    CurrentArchitectureBackend::initialize(exception::handler, interrupt::handler);
    roxy_memory::initialize(&boot_info);
    roxy_time::initialize(boot_info.unix_seconds_at_boot);
    rootfs::initialize(&boot_info).expect("initialize root filesystem");
    roxy_cpu::current_cpu().initialize();
    roxy_process::initialize();
    roxy_futex::initialize();
    roxy_syscall::initialize();
    CurrentArchitectureBackend::enable_interrupts();

    #[cfg(feature = "kernel-test")]
    test::run();

    #[cfg(not(feature = "kernel-test"))]
    kernel_main(boot_info)
}

#[cfg(not(feature = "kernel-test"))]
fn kernel_main(_boot_info: BootInfo) -> ! {
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
