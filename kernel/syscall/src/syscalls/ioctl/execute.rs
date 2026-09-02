use roxy_fd::{IoctlError, OpenFile};
use roxy_tty_types::ApplyWhen;

use super::{evdev, framebuffer, terminal};
use crate::errno::Errno;

pub(super) fn execute(file: &OpenFile, raw_request: u64, raw_argument: u64) -> Result<u64, Errno> {
    // evdev requests carry the parameter size and direction inside the request word itself
    // (`_IOC`), so they cannot be matched as exact constants. Dispatch them by type byte first.
    if evdev::is_evioc_request(raw_request) {
        return evdev::execute(file, raw_request, raw_argument);
    }

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
        framebuffer::FBIOGET_VSCREENINFO => {
            framebuffer::get_var_screen_info(file, raw_argument).map(|()| 0)
        }
        framebuffer::FBIOPUT_VSCREENINFO => {
            framebuffer::set_var_screen_info(file, raw_argument).map(|()| 0)
        }
        framebuffer::FBIOGET_FSCREENINFO => {
            framebuffer::get_fix_screen_info(file, raw_argument).map(|()| 0)
        }
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
