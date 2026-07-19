use core::time::Duration;

pub(crate) const TIMER_HZ: u64 = 250;
const TICK_NANOS: u64 = 1_000_000_000 / TIMER_HZ;

pub(crate) fn advance_time() {
    roxy_time::advance(Duration::from_nanos(TICK_NANOS));
}
