use roxy_arch::LocalInterruptKind;

pub(crate) fn handler(kind: LocalInterruptKind) {
    roxy_cpu::handle_local_interrupt(kind);
    if kind == LocalInterruptKind::Timer {
        roxy_thread::scheduler::on_timer_interrupt();
    }
}
