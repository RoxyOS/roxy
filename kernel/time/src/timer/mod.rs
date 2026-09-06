#[cfg(target_arch = "x86_64")]
mod x86_64;

use core::time::Duration;

use roxy_arch::LocalInterruptKind;

#[cfg(target_arch = "x86_64")]
use self::x86_64::X86_64Timer;

#[cfg(target_arch = "x86_64")]
type CurrentTimerBackend = X86_64Timer;

pub(crate) const TIMER_HZ: u64 = 250;
const TICK_NANOS: u64 = 1_000_000_000 / TIMER_HZ;

trait TimerBackend: sealed::Sealed {
    fn initialize();

    fn initialize_ap();

    fn start();
}

pub(super) fn initialize() {
    CurrentTimerBackend::initialize();
    roxy_interrupt::register_local_handler(LocalInterruptKind::Timer, on_tick);
}

pub(super) fn initialize_ap() {
    CurrentTimerBackend::initialize_ap();
    // No handler registration — the BSP already registered the global tick handler.
}

pub(super) fn start() {
    CurrentTimerBackend::start();
}

fn on_tick() {
    super::advance(Duration::from_nanos(TICK_NANOS));
}

mod sealed {
    pub trait Sealed {}
}
