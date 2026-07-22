#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

mod exception;
mod initial_fds;
mod misc;
mod rootfs;
#[cfg(feature = "kernel-test")]
mod test;

use core::{alloc::Layout, panic::PanicInfo};

use roxy_arch::{Architecture, CurrentArchitectureBackend};
use roxy_boot::BootInfo;
use roxy_interrupt::InterruptPlatformInfo;
use roxy_serial::e_println;

#[cfg(not(feature = "kernel-test"))]
const INIT: &[u8] = b"/bin/sh";

#[unsafe(no_mangle)]
#[allow(clippy::missing_panics_doc)]
pub extern "C" fn _start() -> ! {
    misc::clear_bss();
    roxy_serial::initialize();

    let boot_info = BootInfo::parse();

    CurrentArchitectureBackend::initialize(exception::handler);
    roxy_memory::initialize(&boot_info);
    select_kernel_terminal(&boot_info);
    roxy_time::initialize(boot_info.unix_seconds_at_boot);
    rootfs::initialize(&boot_info).expect("initialize root filesystem");
    let rsdp_address = boot_info
        .rsdp_address
        .checked_sub(boot_info.hhdm_offset)
        .expect("Limine RSDP address is outside the HHDM");
    let interrupt_init_result = roxy_interrupt::initialize(InterruptPlatformInfo {
        rsdp_address,
        hhdm_offset: boot_info.hhdm_offset,
    });
    roxy_cpu::current_cpu().initialize(interrupt_init_result.hardware_id());
    roxy_time::initialize_periodic_timer();
    roxy_process::initialize(initial_fds::inject);
    roxy_futex::initialize();
    roxy_syscall::initialize();
    roxy_thread::initialize();
    roxy_time::start_periodic_timer();
    CurrentArchitectureBackend::enable_interrupts();

    #[cfg(feature = "kernel-test")]
    test::run();

    #[cfg(not(feature = "kernel-test"))]
    kernel_main()
}

#[cfg(feature = "kernel-test")]
fn select_kernel_terminal(_boot_info: &BootInfo) {
    roxy_terminal::select_kernel_terminal(roxy_serial::terminal());
}

#[cfg(not(feature = "kernel-test"))]
fn select_kernel_terminal(boot_info: &BootInfo) {
    let terminal = match roxy_fbterm::initialize(boot_info) {
        Ok(()) => roxy_fbterm::terminal().expect("initialized fbterm must publish its endpoint"),
        Err(error) => {
            roxy_serial::e_println!("fbterm unavailable: {error:?}; using serial terminal");
            roxy_serial::terminal()
        }
    };

    roxy_terminal::select_kernel_terminal(terminal);
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
