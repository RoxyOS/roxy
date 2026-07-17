#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

mod exception;
mod interrupt;
mod misc;
mod serial;
#[cfg(feature = "kernel-test")]
mod test;

#[cfg(not(feature = "kernel-test"))]
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
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
    roxy_cpu::current_cpu().initialize();
    CurrentArchitectureBackend::enable_interrupts();

    #[cfg(feature = "kernel-test")]
    test::run();

    #[cfg(not(feature = "kernel-test"))]
    kernel_main(boot_info)
}

#[cfg(not(feature = "kernel-test"))]
fn kernel_main(_boot_info: BootInfo) -> ! {
    roxy_thread::scheduler::spawn(thread_a).unwrap();
    roxy_thread::scheduler::spawn(thread_b).unwrap();
    s_println!("Starting round-robin scheduler");
    roxy_thread::scheduler::start()
}

#[cfg(not(feature = "kernel-test"))]
static THREAD_A_TICKS: AtomicU64 = AtomicU64::new(0);
#[cfg(not(feature = "kernel-test"))]
static THREAD_B_TICKS: AtomicU64 = AtomicU64::new(0);
#[cfg(not(feature = "kernel-test"))]
static ROUND_ROBIN_CONFIRMED: AtomicBool = AtomicBool::new(false);

#[cfg(not(feature = "kernel-test"))]
fn thread_a() -> ! {
    s_println!("Thread A started");
    loop {
        THREAD_A_TICKS.fetch_add(1, Ordering::Relaxed);
        if THREAD_B_TICKS.load(Ordering::Relaxed) != 0
            && !ROUND_ROBIN_CONFIRMED.swap(true, Ordering::Relaxed)
        {
            s_println!("Round-robin A -> B -> A confirmed");
        }
        core::hint::spin_loop();
    }
}

#[cfg(not(feature = "kernel-test"))]
fn thread_b() -> ! {
    s_println!("Thread B started");
    loop {
        THREAD_B_TICKS.fetch_add(1, Ordering::Relaxed);
        core::hint::spin_loop();
    }
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
