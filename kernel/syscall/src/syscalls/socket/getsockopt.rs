use roxy_fd::Fd;
use roxy_memory::UserAddress;
use roxy_process::DescriptorError;

use crate::{SyscallResult, args::user_memory, errno::Errno, numbers::SyscallNumber, syscall};

syscall!(SyscallNumber::GetSockopt, handle(
    fd: Fd => BadFd,
    layer: u64,
    number: u64,
    buffer: UserAddress => Fault,
    size: UserAddress => Fault,
));

fn handle(
    fd: Fd,
    layer: u64,
    number: u64,
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
        .socket_ops(|socket| {
            socket.get_sockopt(
                u32::try_from(layer).unwrap_or(u32::MAX),
                u32::try_from(number).unwrap_or(u32::MAX),
                &mut local_buffer,
            )
        })
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
