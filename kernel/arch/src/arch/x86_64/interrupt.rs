use core::{
    arch::asm,
    sync::atomic::{AtomicUsize, Ordering},
};

use x86_64::structures::idt::InterruptStackFrame;

use crate::{LocalInterruptHandler, LocalInterruptKind};

pub(super) const TIMER_VECTOR: u8 = 0xf0;
pub(super) const ERROR_VECTOR: u8 = 0xfe;
pub(super) const SPURIOUS_VECTOR: u8 = 0xff;

static HANDLER: AtomicUsize = AtomicUsize::new(0);

pub(super) const fn vector(kind: LocalInterruptKind) -> u8 {
    match kind {
        LocalInterruptKind::Timer => TIMER_VECTOR,
        LocalInterruptKind::Error => ERROR_VECTOR,
        LocalInterruptKind::Spurious => SPURIOUS_VECTOR,
    }
}

pub(super) fn register(handler: LocalInterruptHandler) {
    HANDLER.store(handler as usize, Ordering::Release);
}

pub(super) fn wait() {
    // SAFETY: STI takes effect after HLT, and CLI restores the scheduler's interrupt invariant.
    unsafe { asm!("sti", "hlt", "cli", options(nomem, nostack)) };
}

fn dispatch(kind: LocalInterruptKind) {
    let address = HANDLER.load(Ordering::Acquire);
    assert_ne!(address, 0, "local interrupt handler is not registered");
    // SAFETY: register stores a valid LocalInterruptHandler function pointer once.
    let handler: LocalInterruptHandler = unsafe { core::mem::transmute(address) };
    handler(kind);
}

pub(super) extern "x86-interrupt" fn timer(_frame: InterruptStackFrame) {
    dispatch(LocalInterruptKind::Timer);
}

pub(super) extern "x86-interrupt" fn error(_frame: InterruptStackFrame) {
    dispatch(LocalInterruptKind::Error);
}

pub(super) extern "x86-interrupt" fn spurious(_frame: InterruptStackFrame) {
    dispatch(LocalInterruptKind::Spurious);
}
