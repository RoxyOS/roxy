mod pit;

use spin::Once;
use x2apic::lapic::{LocalApic, LocalApicBuilder, TimerDivide, TimerMode};

use roxy_arch::{Architecture, CurrentArchitectureBackend, Interrupt, LocalInterruptKind};
use roxy_cpu::CpuLocal;
use roxy_utils::Lock;

use super::{TIMER_HZ, TimerBackend, sealed};

static APIC_TIMER: CpuLocal<Lock<X2ApicTimer>> = CpuLocal::new();

/// Calibrated LAPIC timer initial count, shared with APs so they skip the PIT-based calibration.
static CALIBRATED_COUNT: Once<u32> = Once::new();

pub(super) struct X86_64Timer;

struct X2ApicTimer {
    local_apic: LocalApic,
}

// SAFETY: The builder receives no xAPIC base, so every successfully built value uses MSR-only x2APIC.
unsafe impl Send for X2ApicTimer {}

impl sealed::Sealed for X86_64Timer {}

impl TimerBackend for X86_64Timer {
    fn initialize() {
        assert!(!CurrentArchitectureBackend::interrupts_enabled());

        let mut local_apic = build_local_apic();
        let initial_count = calibrate_timer(&mut local_apic);
        CALIBRATED_COUNT.call_once(|| initial_count);
        // SAFETY: The timer remains masked while its periodic configuration is installed.
        unsafe {
            local_apic.set_timer_mode(TimerMode::Periodic);
            local_apic.set_timer_divide(TimerDivide::Div16);
            local_apic.set_timer_initial(initial_count);
            local_apic.disable_timer();
        }

        APIC_TIMER.initialize_current(Lock::new(X2ApicTimer { local_apic }));
    }

    fn initialize_ap() {
        assert!(!CurrentArchitectureBackend::interrupts_enabled());

        let initial_count = *CALIBRATED_COUNT
            .get()
            .expect("BSP must calibrate the timer first");
        let mut local_apic = build_local_apic();
        // SAFETY: The timer remains masked while its periodic configuration is installed.
        unsafe {
            local_apic.set_timer_mode(TimerMode::Periodic);
            local_apic.set_timer_divide(TimerDivide::Div16);
            local_apic.set_timer_initial(initial_count);
            local_apic.disable_timer();
        }

        APIC_TIMER.initialize_current(Lock::new(X2ApicTimer { local_apic }));
    }

    fn start() {
        with_apic_timer(|local_apic| {
            // SAFETY: All timer interrupt handlers are registered before unmasking the timer.
            unsafe { local_apic.enable_timer() };
        });
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
        )))
        .timer_mode(TimerMode::OneShot)
        .timer_divide(TimerDivide::Div16)
        .timer_initial(u32::MAX);
    builder.build().unwrap()
}

fn calibrate_timer(local_apic: &mut LocalApic) -> u32 {
    // SAFETY: Interrupts are disabled and the uniquely borrowed timer is used as a one-shot counter.
    unsafe {
        local_apic.set_timer_mode(TimerMode::OneShot);
        local_apic.set_timer_divide(TimerDivide::Div16);
        local_apic.set_timer_initial(u32::MAX);
        local_apic.enable_timer();
    }
    pit::wait_calibration_window();
    // SAFETY: The timer is uniquely borrowed and still contains the calibration countdown.
    let current = unsafe { local_apic.timer_current() };
    // SAFETY: Calibration is complete; masking prevents an interrupt before CPU-local state exists.
    unsafe { local_apic.disable_timer() };

    periodic_initial_count(u32::MAX - current).expect("invalid local APIC timer calibration")
}

fn periodic_initial_count(elapsed: u32) -> Option<u32> {
    let numerator = u64::from(elapsed).checked_mul(pit::FREQUENCY_HZ)?;
    let denominator = u64::from(pit::CALIBRATION_RELOAD).checked_mul(TIMER_HZ)?;
    let count = numerator.checked_div(denominator)?;
    u32::try_from(count).ok().filter(|count| *count != 0)
}

fn with_apic_timer<T>(function: impl FnOnce(&mut LocalApic) -> T) -> T {
    assert!(!CurrentArchitectureBackend::interrupts_enabled());
    function(&mut APIC_TIMER.get().lock().local_apic)
}

#[cfg(feature = "kernel-test")]
mod tests {
    use super::periodic_initial_count;

    roxy_test::kernel_test!(
        "roxy-time::apic-calibration-conversion",
        apic_calibration_conversion,
        {
            assert_eq!(periodic_initial_count(0), None);
            assert_eq!(periodic_initial_count(14_914_750), Some(1_193_182));
            assert!(periodic_initial_count(u32::MAX).is_some());
        }
    );
}
