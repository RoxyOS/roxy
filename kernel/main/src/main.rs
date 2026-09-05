#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

#[cfg(not(feature = "kernel-test"))]
use alloc::vec::Vec;

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
const INIT: &[u8] = b"/usr/bin/bash";

/// Environment for the init process. `HOME` is required so login shells (xinit and its
/// `.xinitrc` lookup) can resolve the user's home directory.
#[cfg(not(feature = "kernel-test"))]
const INIT_ENV: &[&[u8]] = &[b"HOME=/root", b"PATH=/usr/bin:/bin", b"TERM=linux"];

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
    let device_registry = rootfs::initialize(&boot_info).expect("initialize root filesystem");
    roxy_fbdev::register(&device_registry);
    roxy_devfs::register_null(&device_registry);
    let pty_registry = roxy_pty::registry();
    device_registry
        .register(b"ptmx", pty_registry.clone())
        .expect("pty master registered exactly once");
    device_registry.register_dynamic_resolver(pty_registry.clone());
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
    roxy_timer_wait::initialize();
    roxy_posix_timer::initialize();
    roxy_thread::initialize();
    roxy_ps2::initialize();
    roxy_ps2::register_psaux(&device_registry);
    let (keyboard_event, keyboard_listener) = roxy_evdev_keyboard::create();
    device_registry
        .register(b"keyboard_event", keyboard_event)
        .expect("keyboard evdev device registered exactly once");
    let keyboard_listener: alloc::sync::Arc<dyn roxy_keyboard_input::KeyboardListener> =
        keyboard_listener;
    let (mouse_event, mouse_listener) = roxy_evdev_mouse::create(roxy_ps2::mouse_has_wheel());
    device_registry
        .register(b"mouse_event", mouse_event)
        .expect("mouse evdev device registered exactly once");
    let mouse_listener: alloc::sync::Arc<dyn roxy_mouse_input::MouseListener> = mouse_listener;
    let tty = roxy_tty::initialize(roxy_terminal::kernel_terminal());
    roxy_keyboard_input::register_listener(&keyboard_listener);
    roxy_mouse_input::register_listener(&mouse_listener);
    roxy_keyboard_input::register_listener(&tty);
    roxy_tty::register_console_device(&device_registry);
    roxy_process::initialize(initial_fds::inject);
    roxy_futex::initialize();
    roxy_syscall::initialize();
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
    use alloc::sync::Arc;

    let terminal = match roxy_fbterm::initialize(boot_info) {
        Ok(()) => {
            let fb = roxy_fbterm::terminal().expect("initialized fbterm must publish its endpoint");
            let serial = roxy_serial::terminal();
            Arc::new(roxy_terminal::TeeOutput::new(fb, serial))
                as Arc<dyn roxy_terminal::TerminalOutput>
        }
        Err(error) => {
            roxy_serial::e_println!("fbterm unavailable: {error:?}; using serial terminal");
            roxy_serial::terminal()
        }
    };

    roxy_terminal::select_kernel_terminal(terminal);
}

#[cfg(not(feature = "kernel-test"))]
fn kernel_main() -> ! {
    let init = roxy_process::spawn(
        INIT,
        &INIT_ENV.iter().map(|s| s.to_vec()).collect::<Vec<_>>(),
    )
    .expect("spawn init process");

    // The kernel plays the login/getty role for the initial shell: bind the terminal to the
    // init process's session and make its process group the initial foreground group. Bash can
    // then run job control against a valid foreground group.
    let init_pgid = roxy_process::process_pgid(init).expect("init process has a process group");
    let init_session =
        roxy_process::process_session_id(init).expect("init process must be a session leader");
    roxy_tty::bind_controlling_terminal(init_session, init_pgid);

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
