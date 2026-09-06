#![no_std]

mod arch;
mod dispatch;
mod misc;
mod registry;
mod state;

use roxy_arch::{Architecture, CpuId, CurrentArchitectureBackend, IrqLine, LocalInterruptKind};

use arch::InterruptBackend;

pub use state::{InterruptStatistics, current_statistics};

/// Handler invoked for a local interrupt or external IRQ while interrupts remain disabled.
///
/// Handlers must acknowledge their device and avoid blocking, switching threads, or re-enabling
/// interrupts.
pub type Handler = fn();

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InterruptPlatformInfo {
    /// Physical address of the ACPI RSDP.
    pub rsdp_address: u64,
    /// Virtual offset of Limine's higher-half direct map.
    pub hhdm_offset: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InterruptInitResult {
    hardware_id: u32,
}

impl InterruptInitResult {
    #[must_use]
    pub const fn hardware_id(self) -> u32 {
        self.hardware_id
    }
}

/// Initializes the current CPU's local interrupt controller and accounting state.
///
/// # Panics
///
/// Panics when interrupts are enabled, initialization is repeated, or controller setup fails.
pub fn initialize(platform: InterruptPlatformInfo) -> InterruptInitResult {
    assert!(!CurrentArchitectureBackend::interrupts_enabled());

    CurrentArchitectureBackend::register_interrupt_dispatcher(dispatch::handle);
    let hardware_id = arch::CurrentInterruptBackend::initialize(platform);
    state::INTERRUPT_STATE.initialize_current(state::InterruptState::new());
    registry::register_local(LocalInterruptKind::Error, misc::record_apic_error);
    registry::register_local(LocalInterruptKind::Spurious, misc::record_spurious);

    InterruptInitResult { hardware_id }
}

/// Initialises the current application processor's local interrupt controller and accounting
/// state, without the BSP-only IOAPIC/dispatcher setup (the BSP already owns those).
pub fn initialize_ap() -> u32 {
    assert!(!CurrentArchitectureBackend::interrupts_enabled());

    let hardware_id = arch::CurrentInterruptBackend::initialize_ap();
    state::INTERRUPT_STATE.initialize_current(state::InterruptState::new());
    hardware_id
}

/// Registers one consumer for a local interrupt kind.
///
/// # Panics
///
/// Panics when interrupts are enabled, the same handler is already registered, or the handler list
/// is full.
pub fn register_local_handler(kind: LocalInterruptKind, handler: Handler) {
    assert!(!CurrentArchitectureBackend::interrupts_enabled());
    registry::register_local(kind, handler);
}

/// Registers one consumer for an external IRQ line.
///
/// # Panics
///
/// Panics when interrupts are enabled, the handler is duplicated, or the fixed handler capacity
/// is exhausted.
pub fn register_irq_handler(line: IrqLine, handler: Handler) {
    assert!(!CurrentArchitectureBackend::interrupts_enabled());
    registry::register_irq(line, handler);
}

/// Sends a reschedule IPI to wake an idle application processor.
///
/// The target does not need a registered handler for the reschedule vector: delivery alone wakes
/// it out of `wait_for_interrupt`, after which the interrupt is EOI'd and the target re-enters its
/// dispatch loop.
///
/// # Panics
///
/// Panics when interrupts are enabled or `target` is not a registered CPU.
pub fn send_reschedule_ipi(target: CpuId) {
    assert!(!CurrentArchitectureBackend::interrupts_enabled());
    arch::CurrentInterruptBackend::send_ipi(target);
}

/// Enables delivery for an external IRQ line after its handler is registered.
///
/// # Panics
///
/// Panics when interrupts are enabled or the line is outside the configured IOAPIC.
pub fn unmask_irq(line: IrqLine) {
    assert!(!CurrentArchitectureBackend::interrupts_enabled());
    arch::CurrentInterruptBackend::unmask_irq(line);
}

/// Disables delivery for an external IRQ line.
///
/// # Panics
///
/// Panics when interrupts are enabled or the line is outside the configured IOAPIC.
pub fn mask_irq(line: IrqLine) {
    assert!(!CurrentArchitectureBackend::interrupts_enabled());
    arch::CurrentInterruptBackend::mask_irq(line);
}
