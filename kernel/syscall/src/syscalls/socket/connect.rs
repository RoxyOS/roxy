use roxy_fd::Fd;
use roxy_memory::UserAddress;
use roxy_process::DescriptorError;

use crate::{SyscallResult, errno::Errno, numbers::SyscallNumber, syscall};

syscall!(SyscallNumber::Connect, handle(fd: Fd => BadFd, address: UserAddress => Fault, length: u64));

fn handle(fd: Fd, address: UserAddress, length: u64) -> SyscallResult {
    let file = roxy_process::current_open_file(fd).map_err(map_descriptor_error)?;

    let path = super::decode_socket_path(address, length)?;

    file.socket_ops(|socket| socket.connect(&path))
        .ok_or(Errno::NotSocket)?
        .map_err(super::map_socket_error)?;

    Ok(0)
}

fn map_descriptor_error(_: DescriptorError) -> Errno {
    Errno::BadFd
}
