use roxy_fd::{IoctlRequest, OpenFile};
use roxy_memory::UserAddress;
use roxy_tty_types::{ApplyWhen, Termios, WindowSize};

use super::terminal_abi;
use crate::{
    args::{Out, SyscallArg, user_memory},
    errno::Errno,
};

pub(super) const TCGETS: u64 = 0x5401;
pub(super) const TCSETS: u64 = 0x5402;
pub(super) const TCSETSW: u64 = 0x5403;
pub(super) const TCSETSF: u64 = 0x5404;
pub(super) const TIOCGWINSZ: u64 = 0x5413;
pub(super) const TIOCSWINSZ: u64 = 0x5414;
pub(super) const TIOCGPGRP: u64 = 0x540f;
pub(super) const TIOCSPGRP: u64 = 0x5410;
pub(super) const TIOCSCTTY: u64 = 0x540e;

pub(super) fn get_termios(file: &OpenFile, raw_argument: u64) -> Result<(), Errno> {
    let address = UserAddress::parse(raw_argument, Errno::Fault)?;
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
    raw_argument: u64,
) -> Result<(), Errno> {
    let address = UserAddress::parse(raw_argument, Errno::Fault)?;
    let termios = terminal_abi::read_termios(address)?;

    file.ioctl(IoctlRequest::SetTermios { when, termios })
        .map_err(super::execute::map_ioctl_error)
}

pub(super) fn get_window_size(file: &OpenFile, raw_argument: u64) -> Result<(), Errno> {
    let address = UserAddress::parse(raw_argument, Errno::Fault)?;
    let output = Out::<terminal_abi::WindowSizeAbi>::parse(address.as_u64(), Errno::Fault)?;
    output.validate()?;
    let mut window_size = WindowSize::default();

    file.ioctl(IoctlRequest::GetWindowSize(&mut window_size))
        .map_err(super::execute::map_ioctl_error)?;
    terminal_abi::write_window_size(output, window_size)
}

pub(super) fn set_window_size(file: &OpenFile, raw_argument: u64) -> Result<(), Errno> {
    let address = UserAddress::parse(raw_argument, Errno::Fault)?;
    let window_size = terminal_abi::read_window_size(address)?;

    file.ioctl(IoctlRequest::SetWindowSize(window_size))
        .map_err(super::execute::map_ioctl_error)
}

pub(super) fn get_foreground_pgid(file: &OpenFile, raw_argument: u64) -> Result<(), Errno> {
    let address = UserAddress::parse(raw_argument, Errno::Fault)?;
    let output = Out::<u32>::parse(address.as_u64(), Errno::Fault)?;
    output.validate()?;
    let mut pgid = 0u32;

    file.ioctl(IoctlRequest::GetForegroundPgid(&mut pgid))
        .map_err(super::execute::map_ioctl_error)?;

    // SAFETY: u32 has no padding and pgid is initialized.
    unsafe { output.write(&pgid) }?;

    Ok(())
}

pub(super) fn set_foreground_pgid(file: &OpenFile, raw_argument: u64) -> Result<(), Errno> {
    let address = UserAddress::parse(raw_argument, Errno::Fault)?;
    let mut pgid = 0u32;
    // SAFETY: u32 has no padding and every bit pattern is valid.
    unsafe { user_memory::read(address, &mut pgid) }?;

    file.ioctl(IoctlRequest::SetForegroundPgid(pgid))
        .map_err(super::execute::map_ioctl_error)
}

pub(super) fn set_controlling_terminal(file: &OpenFile, force: u64) -> Result<(), Errno> {
    // TIOCSCTTY: the calling process makes its own session the controller of this terminal,
    // binding `owner_session_id` and the initial foreground process group to the caller's
    // session. The kernel-side terminal enforces that the caller is a session leader.
    file.ioctl(IoctlRequest::SetControllingTerminal { force: force != 0 })
        .map_err(super::execute::map_ioctl_error)
}
