use roxy_fd::Fd;

use crate::{SyscallResult, errno::Errno, numbers::SyscallNumber, syscall};

use super::{
    map_file_error,
    msg::{MsgFlags, ParsedMsgHdr, read_iovecs, recvmsg_scatter},
};

syscall!(SyscallNumber::RecvMsg, handle(
    fd: Fd => BadFd,
    header: ParsedMsgHdr => Fault,
    flags: MsgFlags => Invalid,
));

fn handle(fd: Fd, header: ParsedMsgHdr, flags: MsgFlags) -> SyscallResult {
    let file = roxy_process::current_open_file(fd).map_err(|_| Errno::BadFd)?;

    // Parse the iovec array.
    let iovecs = read_iovecs(header.msg_iov(), header.msg_iovlen())?;

    // MSG_DONTWAIT overrides the per-file nonblocking flag.
    let nonblocking = flags.contains(MsgFlags::DONTWAIT);

    // Read and scatter. Note: msg_control is a caller-allocated buffer for receiving ancillary
    // data (SCM_RIGHTS); since Roxy does not deliver control messages yet, it is left untouched
    // and msg_controllen is reported as zero below, rather than rejected.
    let read = recvmsg_scatter(&file, &iovecs, nonblocking).map_err(map_file_error)?;

    // Write back msg_controllen = 0 (no control messages received).
    header.write_controllen(0)?;

    // Write back msg_flags = 0 (no ancillary flags set).
    header.write_flags(0)?;

    Ok(u64::try_from(read).unwrap())
}
