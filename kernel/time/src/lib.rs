#![no_std]

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

/// Advances the global monotonic clock by one hardware-provided interval.
pub fn advance(elapsed: Duration) {
    let elapsed_nanos = u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX);
    let _ = MONOTONIC_NANOS.try_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
        Some(current.saturating_add(elapsed_nanos))
    });
}

#[must_use]
pub fn monotonic_time() -> Duration {
    Duration::from_nanos(MONOTONIC_NANOS.load(Ordering::Relaxed))
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
