use roxy_fd::{Fd, FileError};
use roxy_memory::UserAddress;
use roxy_process::{self, DescriptorError};
use roxy_signal::Signal;

use crate::{SyscallResult, args::Slice, errno::Errno, numbers::SyscallNumber, syscall};

syscall!(SyscallNumber::Write, handle(fd: Fd => BadFd, address: UserAddress => Fault, count: usize => Fault));

const BUFFER_SIZE: usize = 4096;

fn handle(fd: Fd, address: UserAddress, count: usize) -> SyscallResult {
    if count == 0 {
        return Ok(0);
    }

    let file = roxy_process::current_open_file(fd).map_err(map_process_error)?;
    let input = Slice::<u8>::new(address, count);
    input.validate()?;
    let mut transferred = 0;

    while transferred < count {
        let remaining = input.skip(transferred)?;
        // SAFETY: u8 has no padding and every bit pattern is valid.
        let buffer = unsafe { remaining.read_with_limit(BUFFER_SIZE) }?;

        let written = match file.write(&buffer) {
            Ok(written) => written,
            Err(FileError::BrokenPipe) => {
                let _ = roxy_process::send_signal(
                    roxy_process::current_process_id(),
                    Signal::BrokenPipe,
                );
                return Err(Errno::Pipe);
            }
            Err(error) => return Err(map_file_error(error)),
        };
        transferred += written;

        if written < buffer.len() {
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
        FileError::WouldBlock => Errno::Again,
        FileError::BadOperation => Errno::BadFd,
        FileError::BrokenPipe => Errno::Pipe,
        FileError::NotConnected => Errno::NotConnected,
        FileError::Io => Errno::Io,
        FileError::Interrupted => Errno::Interrupted,
    }
}
