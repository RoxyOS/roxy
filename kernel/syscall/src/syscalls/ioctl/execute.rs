use roxy_fd::{IoctlError, OpenFile};
use roxy_memory::UserAddress;
use roxy_tty_types::ApplyWhen;

use super::{framebuffer, terminal};
use crate::errno::Errno;

pub(super) fn execute(
    file: &OpenFile,
    raw_request: u64,
    argument: UserAddress,
) -> Result<(), Errno> {
    match raw_request {
        terminal::TCGETS => terminal::get_termios(file, argument),
        terminal::TCSETS => terminal::set_termios(file, ApplyWhen::Immediate, argument),
        terminal::TCSETSW => terminal::set_termios(file, ApplyWhen::Drain, argument),
        terminal::TCSETSF => terminal::set_termios(file, ApplyWhen::Flush, argument),
        terminal::TIOCGWINSZ => terminal::get_window_size(file, argument),
        terminal::TIOCSWINSZ => terminal::set_window_size(file, argument),
        framebuffer::FBIOGET_VSCREENINFO => framebuffer::get_var_screen_info(file, argument),
        framebuffer::FBIOGET_FSCREENINFO => framebuffer::get_fix_screen_info(file, argument),
        _ => Err(Errno::NotTty),
    }
}

pub(super) fn map_ioctl_error(error: IoctlError) -> Errno {
    match error {
        IoctlError::NotTty => Errno::NotTty,
        IoctlError::Unsupported {
            operation,
            argument,
        } => crate::unsupported::unsupported_argument(operation, argument, Errno::NotSupported),
    }
}
