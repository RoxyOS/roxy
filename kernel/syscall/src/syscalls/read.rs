use roxy_fd::{Fd, FileError};
use roxy_memory::UserAddress;
use roxy_process::{self, DescriptorError};

use crate::{SyscallResult, args::Slice, errno::Errno, numbers::SyscallNumber, syscall};

syscall!(SyscallNumber::Read, handle(fd: Fd => BadFd, address: UserAddress => Fault, count: usize => Fault));

const BUFFER_SIZE: usize = 4096;

fn handle(fd: Fd, address: UserAddress, count: usize) -> SyscallResult {
    if count == 0 {
        return Ok(0);
    }

    let length = count.min(BUFFER_SIZE);
    let file = roxy_process::current_open_file(fd).map_err(map_process_error)?;
    let output = Slice::<u8>::new(address, length);
    output.validate()?;

    let mut buffer = [0u8; BUFFER_SIZE];
    let read = file.read(&mut buffer[..length]).map_err(map_file_error)?;

    // SAFETY: u8 has no padding and every byte in buffer is initialized.
    unsafe { output.write(&buffer[..read]) }?;

    Ok(u64::try_from(read).unwrap())
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
    }
}
