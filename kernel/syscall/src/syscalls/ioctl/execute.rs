use roxy_fd::OpenFile;
use roxy_memory::UserAddress;
use roxy_tty_types::ApplyWhen;
use roxy_vm::AddrSpaceHandle;

use super::terminal;
use crate::errno::Errno;

pub(super) fn execute(
    file: &OpenFile,
    raw_request: u64,
    argument: UserAddress,
    addrspace: &AddrSpaceHandle,
) -> Result<(), Errno> {
    match raw_request {
        terminal::TCGETS => terminal::get_termios(file, argument, addrspace),
        terminal::TCSETS => terminal::set_termios(file, ApplyWhen::Immediate, argument, addrspace),
        terminal::TCSETSW => terminal::set_termios(file, ApplyWhen::Drain, argument, addrspace),
        terminal::TCSETSF => terminal::set_termios(file, ApplyWhen::Flush, argument, addrspace),
        terminal::TIOCGWINSZ => terminal::get_window_size(file, argument, addrspace),
        terminal::TIOCSWINSZ => terminal::set_window_size(file, argument, addrspace),
        _ => Err(Errno::NotTty),
    }
}
