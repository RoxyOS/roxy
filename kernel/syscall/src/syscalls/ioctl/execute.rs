use roxy_fd::{IoctlError, OpenFile, StatusFlags};
use roxy_memory::UserAddress;
use roxy_tty_types::ApplyWhen;

use super::{framebuffer, numbers, pty, terminal};
use crate::args::{SyscallArg, user_memory};
use crate::errno::Errno;

/// FIONBIO: set/clear the file description's `O_NONBLOCK` from an `int` argument.
pub(super) const FIONBIO: u64 = numbers::DESCRIPTION_BASE;

pub(super) fn execute(file: &OpenFile, raw_request: u64, raw_argument: u64) -> Result<u64, Errno> {
    match raw_request {
        terminal::TCGETS => terminal::get_termios(file, raw_argument).map(|()| 0),
        terminal::TCSETS => {
            terminal::set_termios(file, ApplyWhen::Immediate, raw_argument).map(|()| 0)
        }
        terminal::TCSETSW => {
            terminal::set_termios(file, ApplyWhen::Drain, raw_argument).map(|()| 0)
        }
        terminal::TCSETSF => {
            terminal::set_termios(file, ApplyWhen::Flush, raw_argument).map(|()| 0)
        }
        terminal::TIOCGWINSZ => terminal::get_window_size(file, raw_argument).map(|()| 0),
        terminal::TIOCSWINSZ => terminal::set_window_size(file, raw_argument).map(|()| 0),
        terminal::TIOCGPGRP => terminal::get_foreground_pgid(file, raw_argument).map(|()| 0),
        terminal::TIOCSPGRP => terminal::set_foreground_pgid(file, raw_argument).map(|()| 0),
        terminal::TIOCSCTTY => terminal::set_controlling_terminal(file, raw_argument).map(|()| 0),
        terminal::TCFLSH => terminal::tcflush(file, raw_argument).map(|()| 0),
        pty::TIOCGPTN => pty::get_pty_number(file, raw_argument).map(|()| 0),
        pty::TIOCSPTLCK => pty::set_pty_lock(file, raw_argument).map(|()| 0),
        framebuffer::ROXY_FRAMEBUFFER_GET_INFO => {
            framebuffer::get_info(file, raw_argument).map(|()| 0)
        }
        framebuffer::ROXY_FRAMEBUFFER_TAKE_CONTROL => framebuffer::take_control(file).map(|()| 0),
        framebuffer::ROXY_FRAMEBUFFER_RELEASE_CONTROL => {
            framebuffer::release_control(file).map(|()| 0)
        }
        FIONBIO => set_nonblocking(file, raw_argument).map(|()| 0),
        _ => Err(unknown_request(raw_request)),
    }
}

/// Reports a request no arm served, keeping the `ENOTTY` a probe expects from a device that does
/// not serve it.
///
/// A request outside the Roxy space carries another personality's numbering — the caller was
/// compiled against another libc's header, or hardcoded one of its constants — and is reported
/// apart from a request inside the space that this kernel simply does not define, because only the
/// first says the caller was built against a different ABI.
fn unknown_request(request: u64) -> Errno {
    if numbers::in_space(request) {
        crate::unsupported::unsupported_argument("ioctl.request", request, Errno::NotTty)
    } else {
        crate::unsupported::unsupported_argument("ioctl.request.foreign", request, Errno::NotTty)
    }
}

fn set_nonblocking(file: &OpenFile, raw_argument: u64) -> Result<(), Errno> {
    let address = UserAddress::parse(raw_argument, Errno::Fault)?;
    let mut arg = 0i32;
    // SAFETY: i32 has no padding and every bit pattern is valid.
    unsafe { user_memory::read(address, &mut arg) }?;

    let mut flags = file.status_flags();
    flags.set(StatusFlags::NONBLOCK, arg != 0);
    file.set_status_flags(flags);

    Ok(())
}

pub(super) fn map_ioctl_error(error: IoctlError) -> Errno {
    match error {
        IoctlError::NotTty => Errno::NotTty,
        IoctlError::Invalid => Errno::Invalid,
        IoctlError::Busy => Errno::Busy,
        IoctlError::Unsupported {
            operation,
            argument,
        } => crate::unsupported::unsupported_argument(operation, argument, Errno::NotSupported),
    }
}
