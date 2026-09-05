//! The POSIX-timer family: `timer_create`, `timer_settime`, `timer_gettime`,
//! `timer_getoverrun`, `timer_delete`.
//!
//! ABI records (`itimerspec`, `sigevent`, the `timer_t` handle encoding) are private to this
//! module. Handlers decode them into ABI-neutral values and delegate to `roxy-posix-timer`.

use crate::errno::Errno;

pub(super) const CREATE_SYSCALL: crate::Syscall = create::SYSCALL;
pub(super) const SETTIME_SYSCALL: crate::Syscall = settime::SYSCALL;
pub(super) const GETTIME_SYSCALL: crate::Syscall = gettime::SYSCALL;
pub(super) const GETOVERRUN_SYSCALL: crate::Syscall = getoverrun::SYSCALL;
pub(super) const DELETE_SYSCALL: crate::Syscall = delete::SYSCALL;

mod abi;
mod create;
mod delete;
mod getoverrun;
mod gettime;
mod settime;

/// Maps the ABI-neutral POSIX-timer error to its ABI errno.
fn map_error(error: roxy_posix_timer::PosixTimerError) -> Errno {
    match error {
        roxy_posix_timer::PosixTimerError::NoEntry => Errno::Invalid,
    }
}
