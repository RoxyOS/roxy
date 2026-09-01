use roxy_fd::{IoctlError, OpenFile};
use roxy_tty_types::ApplyWhen;

use super::{framebuffer, terminal};
use crate::errno::Errno;

pub(super) fn execute(file: &OpenFile, raw_request: u64, raw_argument: u64) -> Result<(), Errno> {
    match raw_request {
        terminal::TCGETS => terminal::get_termios(file, raw_argument),
        terminal::TCSETS => terminal::set_termios(file, ApplyWhen::Immediate, raw_argument),
        terminal::TCSETSW => terminal::set_termios(file, ApplyWhen::Drain, raw_argument),
        terminal::TCSETSF => terminal::set_termios(file, ApplyWhen::Flush, raw_argument),
        terminal::TIOCGWINSZ => terminal::get_window_size(file, raw_argument),
        terminal::TIOCSWINSZ => terminal::set_window_size(file, raw_argument),
        terminal::TIOCGPGRP => terminal::get_foreground_pgid(file, raw_argument),
        terminal::TIOCSPGRP => terminal::set_foreground_pgid(file, raw_argument),
        terminal::TIOCSCTTY => terminal::set_controlling_terminal(file, raw_argument),
        framebuffer::FBIOGET_VSCREENINFO => framebuffer::get_var_screen_info(file, raw_argument),
        framebuffer::FBIOPUT_VSCREENINFO => framebuffer::set_var_screen_info(file, raw_argument),
        framebuffer::FBIOGET_FSCREENINFO => framebuffer::get_fix_screen_info(file, raw_argument),
        _ => Err(Errno::NotTty),
    }
}

pub(super) fn map_ioctl_error(error: IoctlError) -> Errno {
    match error {
        IoctlError::NotTty => Errno::NotTty,
        IoctlError::Invalid => Errno::Invalid,
        IoctlError::Unsupported {
            operation,
            argument,
        } => crate::unsupported::unsupported_argument(operation, argument, Errno::NotSupported),
    }
}
