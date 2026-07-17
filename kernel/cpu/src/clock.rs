use core::sync::atomic::Ordering;

use crate::cpu::CPU_STATE;

pub(crate) const TIMER_HZ: u64 = 250;
pub(crate) const TICK_NANOS: u64 = 1_000_000_000 / TIMER_HZ;

pub(crate) fn advance() {
    let clock = &CPU_STATE.get().monotonic_nanos;
    clock
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            Some(advanced_nanos(current))
        })
        .unwrap();
}

const fn advanced_nanos(current: u64) -> u64 {
    match current.checked_add(TICK_NANOS) {
        Some(next) => next,
        None => u64::MAX,
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use super::{TICK_NANOS, advanced_nanos};

    roxy_test::kernel_test!("roxy-cpu::monotonic-clock-arithmetic", clock_arithmetic, {
        assert_eq!(TICK_NANOS, 4_000_000);
        assert_eq!(advanced_nanos(0), TICK_NANOS);
        assert_eq!(advanced_nanos(u64::MAX - TICK_NANOS + 1), u64::MAX);
        assert_eq!(advanced_nanos(u64::MAX), u64::MAX);
    });
}
