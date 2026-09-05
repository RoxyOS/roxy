mod ioapic;

use x2apic::lapic::{LocalApic, LocalApicBuilder};

use roxy_arch::{Architecture, CurrentArchitectureBackend, Interrupt, IrqLine, LocalInterruptKind};
use roxy_cpu::CpuLocal;
use roxy_utils::Lock;

use super::{InterruptBackend, sealed};
use crate::InterruptPlatformInfo;

static LOCAL_APIC: CpuLocal<Lock<X2Apic>> = CpuLocal::new();

pub(crate) struct X86_64Interrupt;

struct X2Apic {
    local_apic: LocalApic,
}

// SAFETY: The builder receives no xAPIC base, so every successfully built value uses MSR-only x2APIC.
unsafe impl Send for X2Apic {}

impl sealed::Sealed for X86_64Interrupt {}

impl InterruptBackend for X86_64Interrupt {
    fn initialize(platform: InterruptPlatformInfo) -> u32 {
        assert!(!CurrentArchitectureBackend::interrupts_enabled());

        let (local_apic, hardware_id) = build_enabled_local_apic();
        // SAFETY: This is the BSP's unique local controller and interrupts remain disabled.
        unsafe {
            assert!(local_apic.is_bsp());
        }
        ioapic::initialize(platform, hardware_id);
        LOCAL_APIC.initialize_current(Lock::new(X2Apic { local_apic }));
        hardware_id
    }

    fn initialize_ap() -> u32 {
        assert!(!CurrentArchitectureBackend::interrupts_enabled());

        let (local_apic, hardware_id) = build_enabled_local_apic();
        LOCAL_APIC.initialize_current(Lock::new(X2Apic { local_apic }));
        hardware_id
    }

    fn end_of_interrupt() {
        with_local_apic(|local_apic| {
            // SAFETY: This is called once for a delivered non-spurious local APIC interrupt.
            unsafe { local_apic.end_of_interrupt() };
        });
    }

    fn error_flags() -> u8 {
        with_local_apic(|local_apic| {
            // SAFETY: Reading the CPU-local APIC error register has no ownership side effects.
            unsafe { local_apic.error_flags().bits() }
        })
    }

    fn mask_irq(line: IrqLine) {
        ioapic::mask(line);
    }

    fn unmask_irq(line: IrqLine) {
        ioapic::unmask(line);
    }
}

fn build_local_apic() -> LocalApic {
    let mut builder = LocalApicBuilder::new();
    builder
        .timer_vector(usize::from(CurrentArchitectureBackend::interrupt_vector(
            Interrupt::Local(LocalInterruptKind::Timer),
        )))
        .error_vector(usize::from(CurrentArchitectureBackend::interrupt_vector(
            Interrupt::Local(LocalInterruptKind::Error),
        )))
        .spurious_vector(usize::from(CurrentArchitectureBackend::interrupt_vector(
            Interrupt::Local(LocalInterruptKind::Spurious),
        )));
    builder.build().unwrap()
}

/// Builds and enables this CPU's local APIC (timer masked), returning it with its hardware id.
fn build_enabled_local_apic() -> (LocalApic, u32) {
    let mut local_apic = build_local_apic();
    // SAFETY: The local controller is uniquely borrowed and interrupts remain disabled.
    unsafe {
        local_apic.enable();
        local_apic.disable_timer();
    }
    // SAFETY: The local controller is enabled and uniquely borrowed.
    let hardware_id = unsafe { local_apic.id() };
    (local_apic, hardware_id)
}

fn with_local_apic<T>(function: impl FnOnce(&mut LocalApic) -> T) -> T {
    assert!(!CurrentArchitectureBackend::interrupts_enabled());
    function(&mut LOCAL_APIC.get().lock().local_apic)
}
