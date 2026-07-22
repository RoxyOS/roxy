#![no_std]

mod timer;

use core::{
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use spin::Once;

static MONOTONIC_NANOS: AtomicU64 = AtomicU64::new(0);
static UNIX_SECONDS_AT_BOOT: Once<u64> = Once::new();

/// Initializes the realtime base.
pub fn initialize(unix_seconds_at_boot: u64) {
    UNIX_SECONDS_AT_BOOT.call_once(|| unix_seconds_at_boot);
}

/// Initializes the current CPU's periodic timer backend.
///
/// # Panics
///
/// Panics when interrupts are enabled, the backend is initialized twice, or timer calibration
/// fails.
pub fn initialize_periodic_timer() {
    timer::initialize();
}

/// Starts periodic timer interrupts for the current CPU.
///
/// # Panics
///
/// Panics when interrupts are enabled or the timer backend is uninitialized.
pub fn start_periodic_timer() {
    timer::start();
}

/// Advances the global monotonic clock by one hardware-provided interval.
pub(crate) fn advance(elapsed: Duration) {
    let elapsed_nanos = u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX);
    let _ = MONOTONIC_NANOS.try_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
        Some(current.saturating_add(elapsed_nanos))
    });
}

#[must_use]
pub fn monotonic_time() -> Duration {
    Duration::from_nanos(MONOTONIC_NANOS.load(Ordering::Relaxed))
}

#[cfg(feature = "kernel-test")]
mod tests {
    use core::time::Duration;

    use roxy_arch::{Architecture, CurrentArchitectureBackend};
    use roxy_interrupt::current_statistics;

    use super::{monotonic_time, timer::TIMER_HZ};

    roxy_test::kernel_test!("roxy-time::periodic-timer-progresses", periodic_timer, {
        assert!(CurrentArchitectureBackend::interrupts_enabled());
        let before_time = monotonic_time();
        let before = current_statistics();
        let expected_elapsed = Duration::from_nanos(3 * 1_000_000_000 / TIMER_HZ);

        while monotonic_time() < before_time + expected_elapsed {
            CurrentArchitectureBackend::halt();
        }

        let after = current_statistics();
        assert!(after.interrupt_entries >= before.interrupt_entries + 3);
        assert_eq!(after.apic_errors, 0);
        assert_eq!(after.last_apic_error, 0);
    });
}

/// Returns Unix time derived from the boot realtime and monotonic elapsed time.
///
/// # Panics
///
/// Panics when the realtime base has not been initialized.
#[must_use]
pub fn realtime_time() -> Duration {
    let unix_seconds_at_boot = *UNIX_SECONDS_AT_BOOT
        .get()
        .expect("time subsystem must be initialized");

    Duration::from_secs(unix_seconds_at_boot).saturating_add(monotonic_time())
}
