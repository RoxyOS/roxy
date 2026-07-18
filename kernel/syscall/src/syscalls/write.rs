use roxy_fd::{Fd, FileError};
use roxy_memory::UserAddress;
use roxy_process::{self, DescriptorError};

use crate::{Syscall, SyscallResult, errno::Errno, numbers::SyscallNumber};

pub(super) const SYSCALL: Syscall = Syscall::new(SyscallNumber::Write, handle);

const BUFFER_SIZE: usize = 4096;

fn handle(arguments: [u64; 6]) -> SyscallResult {
    let fd = u32::try_from(arguments[0])
        .map(Fd::new)
        .map_err(|_| Errno::BadFd)?;
    let address = UserAddress::new(arguments[1]).ok_or(Errno::Fault)?;
    let count = usize::try_from(arguments[2]).map_err(|_| Errno::Fault)?;

    if count == 0 {
        return Ok(0);
    }

    let file = roxy_process::current_open_file(fd).map_err(map_process_error)?;
    let addrspace = roxy_process::current_addrspace().map_err(map_process_error)?;
    let last_offset = u64::try_from(count - 1).map_err(|_| Errno::Fault)?;

    address.checked_add(last_offset).ok_or(Errno::Fault)?;

    let mut buffer = [0u8; BUFFER_SIZE];
    let mut transferred = 0;

    while transferred < count {
        let length = (count - transferred).min(buffer.len());
        let source = address
            .checked_add(u64::try_from(transferred).unwrap())
            .ok_or(Errno::Fault)?;

        addrspace
            .read_bytes(source, &mut buffer[..length])
            .map_err(|_| Errno::Fault)?;

        let written = file.write(&buffer[..length]).map_err(map_file_error)?;
        transferred += written;

        if written < length {
            break;
        }
    }

    Ok(u64::try_from(transferred).unwrap())
}

fn map_process_error(_: DescriptorError) -> Errno {
    Errno::BadFd
}

fn map_file_error(error: FileError) -> Errno {
    match error {
        FileError::BadOperation => Errno::BadFd,
        FileError::Io => Errno::Io,
    }
}
