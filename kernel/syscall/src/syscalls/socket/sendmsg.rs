use roxy_fd::{Fd, FileError};

use crate::{SyscallResult, args::user_memory, errno::Errno, numbers::SyscallNumber, syscall};

use super::{
    map_file_error,
    msg::{Iovec, MsgFlags, ParsedMsgHdr, read_iovecs},
};

/// Maximum bytes to read from user space in one pass.
const SCRATCH_SIZE: usize = 4096;

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

/// Gathers data from user-space iovecs, then writes it through the file, honoring `nonblocking`.
///
/// Returns the total number of bytes written.
///
/// # Errors
///
/// Returns `FileError::Io` when a user buffer cannot be read, plus any error from the write.
fn write_from_iovec(
    file: &roxy_fd::OpenFile,
    iovecs: &[Iovec],
    nonblocking: bool,
) -> Result<usize, FileError> {
    let mut written = 0usize;

    for iov in iovecs {
        let mut remaining = iov.length;

        while remaining > 0 {
            let chunk = remaining.min(SCRATCH_SIZE);
            let mut buf = [0u8; SCRATCH_SIZE];

            // SAFETY: u8 accepts every byte pattern; the slice is bounded by the iovec length.
            unsafe { user_memory::read_slice(iov.base, &mut buf[..chunk]) }
                .map_err(|_| FileError::Io)?;

            let n = file.write_with_nonblocking(&buf[..chunk], nonblocking)?;
            written += n;

            if n < chunk {
                // Partial write: the file (or nonblocking limit) cannot accept more right now.
                return Ok(written);
            }

            remaining -= chunk;
        }
    }

    Ok(written)
}

fn unsupported(operation: &str, argument: impl core::fmt::Display) -> Errno {
    crate::unsupported::unsupported_argument(operation, argument, Errno::NotSupported)
}
