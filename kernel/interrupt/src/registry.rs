use core::sync::atomic::{AtomicUsize, Ordering};

use roxy_arch::{IrqLine, LocalInterruptKind};

use crate::Handler;

const MAX_HANDLERS: usize = 4;

static REGISTRY: Registry = Registry::new();

struct Registry {
    timer: HandlerList,
    error: HandlerList,
    spurious: HandlerList,
    irq: [HandlerList; IrqLine::ISA_COUNT as usize],
}

impl Registry {
    const fn new() -> Self {
        Self {
            timer: HandlerList::new(),
            error: HandlerList::new(),
            spurious: HandlerList::new(),
            irq: [const { HandlerList::new() }; IrqLine::ISA_COUNT as usize],
        }
    }

    fn local(&self, kind: LocalInterruptKind) -> &HandlerList {
        match kind {
            LocalInterruptKind::Timer => &self.timer,
            LocalInterruptKind::Error => &self.error,
            LocalInterruptKind::Spurious => &self.spurious,
        }
    }

    fn irq(&self, line: IrqLine) -> &HandlerList {
        &self.irq[usize::from(line.number())]
    }
}

struct HandlerList {
    slots: [AtomicUsize; MAX_HANDLERS],
}

impl HandlerList {
    const fn new() -> Self {
        Self {
            slots: [const { AtomicUsize::new(0) }; MAX_HANDLERS],
        }
    }

    fn register(&self, handler: Handler) {
        let address = handler as usize;
        for slot in &self.slots {
            match slot.compare_exchange(0, address, Ordering::AcqRel, Ordering::Acquire) {
                Ok(_) => return,
                Err(existing) if existing == address => {
                    panic!("interrupt handler registered twice")
                }
                Err(_) => {}
            }
        }
        panic!("interrupt handler list is full");
    }

    fn notify(&self) {
        for slot in &self.slots {
            let address = slot.load(Ordering::Acquire);
            if address == 0 {
                continue;
            }
            // SAFETY: all lists accept only Handler function pointers.
            let handler: Handler = unsafe { core::mem::transmute(address) };
            handler();
        }
    }
}

pub(crate) fn register_local(kind: LocalInterruptKind, handler: Handler) {
    REGISTRY.local(kind).register(handler);
}

pub(crate) fn notify_local(kind: LocalInterruptKind) {
    REGISTRY.local(kind).notify();
}

pub(crate) fn register_irq(line: IrqLine, handler: Handler) {
    REGISTRY.irq(line).register(handler);
}

pub(crate) fn notify_irq(line: IrqLine) {
    REGISTRY.irq(line).notify();
}

#[cfg(feature = "kernel-test")]
mod tests {
    use core::sync::atomic::{AtomicUsize, Ordering};

    use super::HandlerList;

    static CALLS: AtomicUsize = AtomicUsize::new(0);

    fn first() {
        CALLS.fetch_add(1, Ordering::Relaxed);
    }

    fn second() {
        CALLS.fetch_add(10, Ordering::Relaxed);
    }

    fn irq_first() {
        CALLS.fetch_add(1, Ordering::Relaxed);
    }

    fn irq_second() {
        CALLS.fetch_add(10, Ordering::Relaxed);
    }

    roxy_test::kernel_test!(
        "roxy-interrupt::handler-list-notifies-in-order",
        handler_list,
        {
            let handlers = HandlerList::new();
            CALLS.store(0, Ordering::Relaxed);

            handlers.register(first);
            handlers.register(second);
            handlers.notify();

            assert_eq!(CALLS.load(Ordering::Relaxed), 11);
        }
    );

    roxy_test::kernel_test!(
        "roxy-interrupt::irq-handlers-notify-in-order",
        irq_handlers,
        {
            let handlers = HandlerList::new();
            CALLS.store(0, Ordering::Relaxed);

            handlers.register(irq_first);
            handlers.register(irq_second);
            handlers.notify();

            assert_eq!(CALLS.load(Ordering::Relaxed), 11);
        }
    );
}
