use core::sync::atomic::{AtomicUsize, Ordering};

use roxy_arch::LocalInterruptKind;

use crate::LocalHandler;

const MAX_HANDLERS: usize = 4;

static REGISTRY: Registry = Registry::new();

struct Registry {
    timer: HandlerList,
    error: HandlerList,
    spurious: HandlerList,
}

impl Registry {
    const fn new() -> Self {
        Self {
            timer: HandlerList::new(),
            error: HandlerList::new(),
            spurious: HandlerList::new(),
        }
    }

    fn list(&self, kind: LocalInterruptKind) -> &HandlerList {
        match kind {
            LocalInterruptKind::Timer => &self.timer,
            LocalInterruptKind::Error => &self.error,
            LocalInterruptKind::Spurious => &self.spurious,
        }
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

    fn register(&self, handler: LocalHandler) {
        let address = handler as usize;

        for slot in &self.slots {
            match slot.compare_exchange(0, address, Ordering::AcqRel, Ordering::Acquire) {
                Ok(_) => return,
                Err(existing) if existing == address => {
                    panic!("local interrupt handler registered twice")
                }
                Err(_) => {}
            }
        }

        panic!("local interrupt handler list is full");
    }

    fn notify(&self) {
        for slot in &self.slots {
            let address = slot.load(Ordering::Acquire);
            if address == 0 {
                continue;
            }
            // SAFETY: register stores only valid LocalHandler function pointers in nonzero slots.
            let handler: LocalHandler = unsafe { core::mem::transmute(address) };
            handler();
        }
    }
}

pub(crate) fn register(kind: LocalInterruptKind, handler: LocalHandler) {
    REGISTRY.list(kind).register(handler);
}

pub(crate) fn notify(kind: LocalInterruptKind) {
    REGISTRY.list(kind).notify();
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
}
