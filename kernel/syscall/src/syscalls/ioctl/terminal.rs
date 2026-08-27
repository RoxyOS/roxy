use roxy_fd::{IoctlRequest, OpenFile};
use roxy_memory::UserAddress;
use roxy_tty_types::{ApplyWhen, Termios, WindowSize};

use super::terminal_abi;
use crate::{
    args::{Out, SyscallArg},
    errno::Errno,
};

pub(super) const TCGETS: u64 = 0x5401;
pub(super) const TCSETS: u64 = 0x5402;
pub(super) const TCSETSW: u64 = 0x5403;
pub(super) const TCSETSF: u64 = 0x5404;
pub(super) const TIOCGWINSZ: u64 = 0x5413;
pub(super) const TIOCSWINSZ: u64 = 0x5414;

pub(super) fn get_termios(file: &OpenFile, address: UserAddress) -> Result<(), Errno> {
    let output = Out::<terminal_abi::TermiosAbi>::parse(address.as_u64(), Errno::Fault)?;
    output.validate()?;
    let mut termios = Termios::default();

    file.ioctl(IoctlRequest::GetTermios(&mut termios))
        .map_err(super::execute::map_ioctl_error)?;
    terminal_abi::write_termios(output, termios)
}

pub(super) fn set_termios(
    file: &OpenFile,
    when: ApplyWhen,
    address: UserAddress,
) -> Result<(), Errno> {
    let termios = terminal_abi::read_termios(address)?;

    file.ioctl(IoctlRequest::SetTermios { when, termios })
        .map_err(super::execute::map_ioctl_error)
}

pub(super) fn get_window_size(file: &OpenFile, address: UserAddress) -> Result<(), Errno> {
    let output = Out::<terminal_abi::WindowSizeAbi>::parse(address.as_u64(), Errno::Fault)?;
    output.validate()?;
    let mut window_size = WindowSize::default();

    file.ioctl(IoctlRequest::GetWindowSize(&mut window_size))
        .map_err(super::execute::map_ioctl_error)?;
    terminal_abi::write_window_size(output, window_size)
}

pub(super) fn set_window_size(file: &OpenFile, address: UserAddress) -> Result<(), Errno> {
    let window_size = terminal_abi::read_window_size(address)?;

    file.ioctl(IoctlRequest::SetWindowSize(window_size))
        .map_err(super::execute::map_ioctl_error)
}
