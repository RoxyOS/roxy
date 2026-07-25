use roxy_fd::{IoctlError, IoctlRequest, OpenFile};
use roxy_memory::UserAddress;
use roxy_tty_types::{ApplyWhen, Termios, WindowSize};
use roxy_vm::AddrSpaceHandle;

use super::terminal_abi;
use crate::{errno::Errno, unsupported::unsupported_argument};

pub(super) const TCGETS: u64 = 0x5401;
pub(super) const TCSETS: u64 = 0x5402;
pub(super) const TCSETSW: u64 = 0x5403;
pub(super) const TCSETSF: u64 = 0x5404;
pub(super) const TIOCGWINSZ: u64 = 0x5413;
pub(super) const TIOCSWINSZ: u64 = 0x5414;

pub(super) fn get_termios(
    file: &OpenFile,
    address: UserAddress,
    addrspace: &AddrSpaceHandle,
) -> Result<(), Errno> {
    validate_output(addrspace, address, terminal_abi::TERMIOS_SIZE)?;
    let mut termios = Termios::default();

    file.ioctl(IoctlRequest::GetTermios(&mut termios))
        .map_err(map_ioctl_error)?;
    terminal_abi::write_termios(addrspace, address, termios)
}

pub(super) fn set_termios(
    file: &OpenFile,
    when: ApplyWhen,
    address: UserAddress,
    addrspace: &AddrSpaceHandle,
) -> Result<(), Errno> {
    let termios = terminal_abi::read_termios(addrspace, address)?;

    file.ioctl(IoctlRequest::SetTermios { when, termios })
        .map_err(map_ioctl_error)
}

pub(super) fn get_window_size(
    file: &OpenFile,
    address: UserAddress,
    addrspace: &AddrSpaceHandle,
) -> Result<(), Errno> {
    validate_output(addrspace, address, terminal_abi::WINDOW_SIZE)?;
    let mut window_size = WindowSize::default();

    file.ioctl(IoctlRequest::GetWindowSize(&mut window_size))
        .map_err(map_ioctl_error)?;
    terminal_abi::write_window_size(addrspace, address, window_size)
}

pub(super) fn set_window_size(
    file: &OpenFile,
    address: UserAddress,
    addrspace: &AddrSpaceHandle,
) -> Result<(), Errno> {
    let window_size = terminal_abi::read_window_size(addrspace, address)?;

    file.ioctl(IoctlRequest::SetWindowSize(window_size))
        .map_err(map_ioctl_error)
}

fn validate_output(
    addrspace: &AddrSpaceHandle,
    address: UserAddress,
    size: usize,
) -> Result<(), Errno> {
    addrspace
        .validate_writable(address, size)
        .map_err(|_| Errno::Fault)
}

fn map_ioctl_error(error: IoctlError) -> Errno {
    match error {
        IoctlError::NotTty => Errno::NotTty,
        IoctlError::Unsupported {
            operation,
            argument,
        } => unsupported_argument(operation, argument, Errno::NotSupported),
    }
}
