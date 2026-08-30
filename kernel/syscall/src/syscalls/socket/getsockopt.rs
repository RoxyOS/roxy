use roxy_fd::{Fd, SockoptLevel, SockoptName};
use roxy_memory::UserAddress;
use roxy_process::DescriptorError;

use crate::{
    SyscallResult,
    args::{SyscallArg, user_memory},
    errno::Errno,
    numbers::SyscallNumber,
    syscall,
};

impl SyscallArg for SockoptLevel {
    fn parse(raw: u64, error: Errno) -> Result<Self, Errno> {
        Self::from_raw(raw).ok_or_else(|| unsupported("getsockopt.level", raw, error))
    }
}

impl SyscallArg for SockoptName {
    fn parse(raw: u64, error: Errno) -> Result<Self, Errno> {
        Self::from_raw(raw).ok_or_else(|| unsupported("getsockopt.optname", raw, error))
    }
}

syscall!(SyscallNumber::GetSockopt, handle(
    fd: Fd => BadFd,
    level: SockoptLevel => Invalid,
    optname: SockoptName => Invalid,
    buffer: UserAddress => Fault,
    size: UserAddress => Fault,
));

fn handle(
    fd: Fd,
    level: SockoptLevel,
    optname: SockoptName,
    buffer: UserAddress,
    size: UserAddress,
) -> SyscallResult {
    let file = roxy_process::current_open_file(fd).map_err(map_descriptor_error)?;

    // The `socklen_t` in-out argument carries the caller's buffer capacity and receives the
    // actual length written.
    let mut max_len = 0u32;
    // SAFETY: u32 has a stable layout with no padding and accepts every bit pattern.
    unsafe { user_memory::read(size, &mut max_len) }?;
    let max_len = usize::try_from(max_len).map_err(|_| Errno::Invalid)?;
    let mut local_buffer = alloc::vec![0u8; max_len];

    let written = file
        .socket_ops(|socket| socket.get_sockopt(level, optname, &mut local_buffer))
        .ok_or(Errno::NotSocket)?
        .map_err(super::map_socket_error)?;

    // SAFETY: Local buffer is initialized and the slice is bounded by `written`.
    unsafe { user_memory::write_slice(buffer, &local_buffer[..written]) }?;

    let actual = u32::try_from(written).map_err(|_| Errno::Overflow)?;
    // SAFETY: `actual` is initialized and `size` points to the caller's socklen_t slot.
    unsafe { user_memory::write(size, &actual) }?;

    Ok(0)
}

fn map_descriptor_error(_: DescriptorError) -> Errno {
    Errno::BadFd
}

fn unsupported(operation: &str, argument: u64, errno: Errno) -> Errno {
    crate::unsupported::unsupported_argument(operation, argument, errno)
}
