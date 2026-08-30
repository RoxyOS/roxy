use roxy_fd::Fd;
use roxy_memory::UserAddress;
use roxy_process::DescriptorError;

use crate::{SyscallResult, args::Out, errno::Errno, numbers::SyscallNumber, syscall};

syscall!(SyscallNumber::Peername, handle(
    fd: Fd => BadFd,
    address: UserAddress => Fault,
    max_len: u64,
    out_len: Out<u32> => Fault,
));

fn handle(fd: Fd, address: UserAddress, max_len: u64, out_len: Out<u32>) -> SyscallResult {
    let file = roxy_process::current_open_file(fd).map_err(map_descriptor_error)?;

    out_len.validate()?;

    let path = file
        .socket_ops(|socket| socket.peername())
        .ok_or(Errno::NotSocket)?
        .map_err(super::map_socket_error)?;

    let total_length = super::encode_socket_path(address, max_len, path.as_deref())?;

    // SAFETY: The u32 value is initialized and the Out slot is validated.
    unsafe { out_len.write(&u32::try_from(total_length).map_err(|_| Errno::Overflow)?) }?;

    Ok(0)
}

fn map_descriptor_error(_: DescriptorError) -> Errno {
    Errno::BadFd
}
