use roxy_fd::OpenFile;
use roxy_memory::UserAddress;
use roxy_tty_types::ApplyWhen;

use super::terminal;
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
        _ => Err(Errno::NotTty),
    }
}
