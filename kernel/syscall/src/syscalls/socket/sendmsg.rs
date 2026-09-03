use roxy_fd::Fd;

use crate::{SyscallResult, errno::Errno, numbers::SyscallNumber, syscall};

use super::{
    map_file_error,
    msg::{MsgFlags, ParsedMsgHdr},
};
use crate::syscalls::iovec::{read_iovecs, write_from_iovec};

syscall!(SyscallNumber::SendMsg, handle(
    fd: Fd => BadFd,
    header: ParsedMsgHdr => Fault,
    flags: MsgFlags => Invalid,
));

fn handle(fd: Fd, header: ParsedMsgHdr, flags: MsgFlags) -> SyscallResult {
    let file = roxy_process::current_open_file(fd).map_err(|_| Errno::BadFd)?;

    // Sending file descriptors (SCM_RIGHTS) is not implemented.
    if header.msg_control().as_u64() != 0 && header.msg_controllen() > 0 {
        return Err(unsupported("sendmsg.control", header.msg_controllen()));
    }

    // For a connected stream socket the destination is implicit; an explicit msg_name is not
    // needed and is ignored (matching Linux for connected sockets).
    let iovecs = read_iovecs(header.msg_iov(), header.msg_iovlen())?;

    // MSG_DONTWAIT overrides the per-file nonblocking flag.
    let nonblocking = flags.contains(MsgFlags::DONTWAIT);

    let written = write_from_iovec(&file, &iovecs, nonblocking).map_err(map_file_error)?;

    Ok(u64::try_from(written).unwrap())
}

fn unsupported(operation: &str, argument: impl core::fmt::Display) -> Errno {
    crate::unsupported::unsupported_argument(operation, argument, Errno::NotSupported)
}
