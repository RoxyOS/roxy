#![no_std]
#![no_main]

mod exception;
mod misc;
mod serial;

use core::panic::PanicInfo;

use roxy_arch::{Architecture, CurrentArchitecture};
use roxy_boot::BootInfo;

#[unsafe(no_mangle)]
#[allow(clippy::missing_panics_doc)]
pub extern "C" fn _start() -> ! {
    misc::clear_bss();
    serial::initialize();

    let boot_info = BootInfo::parse();

    CurrentArchitecture::initialize(exception::handler);

    kernel_main(boot_info)
}

fn kernel_main(_boot_info: BootInfo) -> ! {
    s_println!("Hello world");
    panic!("Reached end of kernel main");
}

#[panic_handler]
fn panic(info: &PanicInfo<'_>) -> ! {
    e_println!("Kernel Panic: {info}");
    CurrentArchitecture::halt_forever()
}
