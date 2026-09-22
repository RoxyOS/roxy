use alloc::vec::Vec;
use core::mem::{align_of, offset_of, size_of};

use roxy_fd::{IoctlRequest, OpenFile};
use roxy_memory::UserAddress;
use roxy_tty_types::{ApplyWhen, Termios, WindowSize};

use super::{numbers, terminal_abi};
use crate::{
    args::{Out, Slice, SyscallArg, user_memory},
    errno::Errno,
};

/// Offsets into the terminal block, in the order the requests were added.
pub(super) const TCGETS: u64 = numbers::TERMINAL_BASE;
pub(super) const TCSETS: u64 = numbers::TERMINAL_BASE + 1;
pub(super) const TCSETSW: u64 = numbers::TERMINAL_BASE + 2;
pub(super) const TCSETSF: u64 = numbers::TERMINAL_BASE + 3;
pub(super) const TIOCGWINSZ: u64 = numbers::TERMINAL_BASE + 4;
pub(super) const TIOCSWINSZ: u64 = numbers::TERMINAL_BASE + 5;
pub(super) const TIOCGPGRP: u64 = numbers::TERMINAL_BASE + 6;
pub(super) const TIOCSPGRP: u64 = numbers::TERMINAL_BASE + 7;
pub(super) const TIOCSCTTY: u64 = numbers::TERMINAL_BASE + 8;
pub(super) const TCFLSH: u64 = numbers::TERMINAL_BASE + 9;
pub(super) const TIOCGNAME: u64 = numbers::TERMINAL_BASE + 10;

/// User ABI for querying the openable path of a terminal.
#[repr(C)]
#[derive(Clone, Copy)]
struct TerminalNameRequestAbi {
    buffer: u64,
    capacity: u64,
    required: u64,
}

const _: () = assert!(size_of::<TerminalNameRequestAbi>() == 24);
const _: () = assert!(align_of::<TerminalNameRequestAbi>() == 8);
const _: () = assert!(offset_of!(TerminalNameRequestAbi, buffer) == 0);
const _: () = assert!(offset_of!(TerminalNameRequestAbi, capacity) == 8);
const _: () = assert!(offset_of!(TerminalNameRequestAbi, required) == 16);

pub(super) fn get_terminal_name(file: &OpenFile, raw_argument: u64) -> Result<(), Errno> {
    let request_address = UserAddress::parse(raw_argument, Errno::Fault)?;
    let mut request = TerminalNameRequestAbi {
        buffer: 0,
        capacity: 0,
        required: 0,
    };
    // SAFETY: TerminalNameRequestAbi has a stable C layout, no padding, and accepts every byte
    // pattern because all fields are integers.
    unsafe { user_memory::read(request_address, &mut request) }?;

    let mut encoded = Vec::new();
    file.ioctl(IoctlRequest::GetTerminalName(&mut encoded))
        .map_err(super::execute::map_ioctl_error)?;

    let required = encoded.len();
    let capacity = usize::try_from(request.capacity).map_err(|_| Errno::Range)?;
    if capacity < required {
        return Err(Errno::Range);
    }

    let output_address = UserAddress::parse(request.buffer, Errno::Fault)?;
    let output = Slice::<u8>::new(output_address, required);
    output.validate()?;

    // SAFETY: u8 has no padding and the ioctl supplied initialized bytes.
    unsafe { output.write(&encoded) }?;

    request.required = u64::try_from(required).map_err(|_| Errno::Overflow)?;
    // SAFETY: request is initialized and its layout has no implicit padding.
    unsafe { user_memory::write(request_address, &request) }?;

    Ok(())
}

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

pub(super) fn tcflush(file: &OpenFile, raw_argument: u64) -> Result<(), Errno> {
    // TCFLSH's argument is the queue selector passed by value (the `int` of tcflush(3)).
    let which = u32::try_from(raw_argument).map_err(|_| Errno::Invalid)?;

    file.ioctl(IoctlRequest::Tcflush(which))
        .map_err(super::execute::map_ioctl_error)
}

pub(super) fn set_controlling_terminal(file: &OpenFile, force: u64) -> Result<(), Errno> {
    // TIOCSCTTY: the calling process makes its own session the controller of this terminal,
    // binding `owner_session_id` and the initial foreground process group to the caller's
    // session. The kernel-side terminal enforces that the caller is a session leader.
    file.ioctl(IoctlRequest::SetControllingTerminal { force: force != 0 })
        .map_err(super::execute::map_ioctl_error)
}
