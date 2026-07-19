mod pit;

use x2apic::lapic::{LocalApic, LocalApicBuilder, TimerDivide, TimerMode};

use roxy_arch::{Architecture, CurrentArchitectureBackend, LocalInterruptKind};
use roxy_utils::Lock;

use crate::{CpuLocal, timer::TIMER_HZ};

use super::{CpuBackend, CpuInitResult, sealed};

static LOCAL_APIC: CpuLocal<Lock<X2Apic>> = CpuLocal::new();

pub(crate) struct X86_64Cpu;

struct X2Apic {
    local_apic: LocalApic,
}

// SAFETY: The builder receives no xAPIC base, so every successfully built value uses MSR-only x2APIC.
unsafe impl Send for X2Apic {}

impl sealed::Sealed for X86_64Cpu {}

impl CpuBackend for X86_64Cpu {
    fn initialize() -> CpuInitResult {
        assert!(!CurrentArchitectureBackend::interrupts_enabled());

        let mut local_apic = build_local_apic();
        // SAFETY: This is the BSP's unique local controller and interrupts remain disabled.
        unsafe {
            local_apic.enable();
            local_apic.disable_timer();
            assert!(local_apic.is_bsp());
        }
        let initial_count = calibrate_timer(&mut local_apic);
        // SAFETY: The timer remains masked while its periodic configuration is installed.
        unsafe {
            local_apic.set_timer_mode(TimerMode::Periodic);
            local_apic.set_timer_divide(TimerDivide::Div16);
            local_apic.set_timer_initial(initial_count);
        }
        // SAFETY: The local controller is enabled and uniquely borrowed.
        let hardware_id = unsafe { local_apic.id() };
        LOCAL_APIC.initialize_current(Lock::new(X2Apic { local_apic }));
        CpuInitResult { hardware_id }
    }

    fn start_timer() {
        with_local_apic(|local_apic| {
            // SAFETY: CPU-local state and the timer handler are ready before unmasking the timer.
            unsafe { local_apic.enable_timer() };
        });
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
}

fn build_local_apic() -> LocalApic {
    let mut builder = LocalApicBuilder::new();
    builder
        .timer_vector(usize::from(
            CurrentArchitectureBackend::local_interrupt_vector(LocalInterruptKind::Timer),
        ))
        .error_vector(usize::from(
            CurrentArchitectureBackend::local_interrupt_vector(LocalInterruptKind::Error),
        ))
        .spurious_vector(usize::from(
            CurrentArchitectureBackend::local_interrupt_vector(LocalInterruptKind::Spurious),
        ))
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

fn with_local_apic<T>(function: impl FnOnce(&mut LocalApic) -> T) -> T {
    assert!(!CurrentArchitectureBackend::interrupts_enabled());
    function(&mut LOCAL_APIC.get().lock().local_apic)
}

#[cfg(feature = "kernel-test")]
mod tests {
    use super::periodic_initial_count;

    roxy_test::kernel_test!(
        "roxy-cpu::apic-calibration-conversion",
        apic_calibration_conversion,
        {
            assert_eq!(periodic_initial_count(0), None);
            assert_eq!(periodic_initial_count(14_914_750), Some(1_193_182));
            assert!(periodic_initial_count(u32::MAX).is_some());
        }
    );
}
