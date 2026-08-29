use roxy_fd::Fd;
use roxy_process::DescriptorError;

use crate::{SyscallResult, errno::Errno, numbers::SyscallNumber, syscall};

syscall!(SyscallNumber::Accept, handle(fd: Fd => BadFd));

fn handle(fd: Fd) -> SyscallResult {
    let file = roxy_process::current_open_file(fd).map_err(map_descriptor_error)?;

    let connection = file
        .socket_ops(|socket| socket.accept())
        .ok_or(Errno::NotSocket)?
        .map_err(super::map_socket_error)?;

    let new_fd = roxy_process::insert_open_file(connection, false);

    Ok(u64::from(new_fd.as_u32()))
}

fn map_descriptor_error(_: DescriptorError) -> Errno {
    Errno::BadFd
}
