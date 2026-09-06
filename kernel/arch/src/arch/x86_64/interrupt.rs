use core::{
    arch::asm,
    sync::atomic::{AtomicUsize, Ordering},
};

use x86_64::structures::idt::InterruptStackFrame;

use crate::{Interrupt, InterruptDispatcher, IrqLine, LocalInterruptKind};

pub(super) const TIMER_VECTOR: u8 = 0xf0;
pub(super) const ERROR_VECTOR: u8 = 0xfe;
pub(super) const SPURIOUS_VECTOR: u8 = 0xff;
pub(super) const IRQ_VECTOR_BASE: u8 = 0x20;
pub(super) const RESCHEDULE_VECTOR: u8 = 0xef;

static HANDLER: AtomicUsize = AtomicUsize::new(0);

pub(super) const fn vector(interrupt: Interrupt) -> u8 {
    match interrupt {
        Interrupt::Local(kind) => match kind {
            LocalInterruptKind::Timer => TIMER_VECTOR,
            LocalInterruptKind::Error => ERROR_VECTOR,
            LocalInterruptKind::Spurious => SPURIOUS_VECTOR,
            LocalInterruptKind::Reschedule => RESCHEDULE_VECTOR,
        },
        Interrupt::Irq(line) => IRQ_VECTOR_BASE + line.number(),
    }
}

pub(super) fn register(handler: InterruptDispatcher) {
    HANDLER.store(handler as usize, Ordering::Release);
}

pub(super) fn wait() {
    // SAFETY: STI takes effect after HLT, and CLI restores the scheduler's interrupt invariant.
    unsafe { asm!("sti", "hlt", "cli", options(nomem, nostack)) };
}

fn dispatch(interrupt: Interrupt) {
    let address = HANDLER.load(Ordering::Acquire);
    assert_ne!(address, 0, "local interrupt handler is not registered");
    // SAFETY: register stores a valid InterruptDispatcher function pointer once.
    let handler: InterruptDispatcher = unsafe { core::mem::transmute(address) };
    handler(interrupt);
}

pub(super) extern "x86-interrupt" fn timer(_frame: InterruptStackFrame) {
    dispatch(Interrupt::Local(LocalInterruptKind::Timer));
}

pub(super) extern "x86-interrupt" fn error(_frame: InterruptStackFrame) {
    dispatch(Interrupt::Local(LocalInterruptKind::Error));
}

pub(super) extern "x86-interrupt" fn spurious(_frame: InterruptStackFrame) {
    dispatch(Interrupt::Local(LocalInterruptKind::Spurious));
}

pub(super) extern "x86-interrupt" fn reschedule(_frame: InterruptStackFrame) {
    dispatch(Interrupt::Local(LocalInterruptKind::Reschedule));
}

macro_rules! irq_stub {
    ($name:ident, $line:literal) => {
        pub(super) extern "x86-interrupt" fn $name(_frame: InterruptStackFrame) {
            dispatch(Interrupt::Irq(IrqLine::new($line).unwrap()));
        }
    };
}

irq_stub!(irq0, 0);
irq_stub!(irq1, 1);
irq_stub!(irq2, 2);
irq_stub!(irq3, 3);
irq_stub!(irq4, 4);
irq_stub!(irq5, 5);
irq_stub!(irq6, 6);
irq_stub!(irq7, 7);
irq_stub!(irq8, 8);
irq_stub!(irq9, 9);
irq_stub!(irq10, 10);
irq_stub!(irq11, 11);
irq_stub!(irq12, 12);
irq_stub!(irq13, 13);
irq_stub!(irq14, 14);
irq_stub!(irq15, 15);
