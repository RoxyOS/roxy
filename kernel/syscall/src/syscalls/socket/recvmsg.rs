use alloc::vec::Vec;

use roxy_fd::{Fd, FileError};

use crate::{SyscallResult, args::user_memory, errno::Errno, numbers::SyscallNumber, syscall};

use super::{
    map_file_error,
    msg::{Iovec, MsgFlags, ParsedMsgHdr, read_iovecs},
};

/// Maximum local buffer for a single recvmsg transfer.
const MAX_RECV_BUFFER: usize = 65536;

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

    // Read, then write the bytes into the caller's iovecs. Note: msg_control is a
    // caller-allocated buffer for receiving ancillary data (SCM_RIGHTS); since Roxy does not
    // deliver control messages yet, it is left untouched and msg_controllen is reported as zero
    // below, rather than rejected.
    let read = read_to_iovec(&file, &iovecs, nonblocking).map_err(map_file_error)?;

    // Write back msg_controllen = 0 (no control messages received).
    header.write_controllen(0)?;

    // Write back msg_flags = 0 (no ancillary flags set).
    header.write_flags(0)?;

    Ok(u64::try_from(read).unwrap())
}

/// Reads through the file (honoring `nonblocking`), then writes the bytes into user iovecs.
///
/// Returns the total number of bytes read.
///
/// # Errors
///
/// Returns `FileError::Io` when a user buffer cannot be written, plus any error from the read.
fn read_to_iovec(
    file: &roxy_fd::OpenFile,
    iovecs: &[Iovec],
    nonblocking: bool,
) -> Result<usize, FileError> {
    if iovecs.is_empty() {
        return Ok(0);
    }

    let total_capacity: usize = iovecs.iter().map(|iov| iov.length).sum();

    if total_capacity == 0 {
        return Ok(0);
    }

    // Read into a local contiguous buffer (bounded), then write it across the iovecs.
    let capacity = total_capacity.min(MAX_RECV_BUFFER);
    let mut buf = Vec::<u8>::new();
    buf.try_reserve_exact(capacity).map_err(|_| FileError::Io)?;
    buf.resize(capacity, 0);

    let read = file.read_with_nonblocking(&mut buf, nonblocking)?;

    if read == 0 {
        return Ok(0);
    }

    let mut offset = 0usize;
    for iov in iovecs {
        if offset >= read {
            break;
        }

        let remaining = read - offset;
        let chunk = remaining.min(iov.length);

        if chunk == 0 {
            continue;
        }

        // SAFETY: buf[offset..offset+chunk] is initialized from the read.
        unsafe { user_memory::write_slice(iov.base, &buf[offset..offset + chunk]) }
            .map_err(|_| FileError::Io)?;

        offset += chunk;
    }

    Ok(read)
}
