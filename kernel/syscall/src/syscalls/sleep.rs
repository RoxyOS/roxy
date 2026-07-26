use crate::{SyscallResult, args::Timespec, numbers::SyscallNumber, syscall};

syscall!(SyscallNumber::Sleep, handle(request: Timespec => Fault));

#[allow(clippy::unnecessary_wraps)]
fn handle(request: Timespec) -> SyscallResult {
    let deadline = roxy_time::monotonic_time().saturating_add(request.duration());

    if deadline > roxy_time::monotonic_time() {
        roxy_timer_wait::block_current(deadline).perform();
    }

    Ok(0)
}
