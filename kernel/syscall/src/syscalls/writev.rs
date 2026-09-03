use roxy_fd::{Fd, FileError};
use roxy_memory::UserAddress;
use roxy_process::{self, DescriptorError};
use roxy_signal::Signal;

use crate::{SyscallResult, errno::Errno, numbers::SyscallNumber, syscall};

use super::iovec::{map_file_error, read_iovecs, write_from_iovec};

/// Maximum number of iovecs accepted by a single writev call (matches Linux `IOV_MAX`).
const IOV_MAX: u32 = 1024;

syscall!(SyscallNumber::Writev, handle(
    fd: Fd => BadFd,
    iovs: UserAddress => Fault,
    iovcnt: u32 => Invalid,
));

fn handle(fd: Fd, iovs: UserAddress, iovcnt: u32) -> SyscallResult {
    if iovcnt > IOV_MAX {
        return Err(Errno::Invalid);
    }

    let file = roxy_process::current_open_file(fd).map_err(map_process_error)?;

    #[allow(clippy::cast_possible_wrap)]
    let iovecs = read_iovecs(iovs, iovcnt as i32)?;

    // A plain writev respects the file's own nonblocking flag: pass `false` so
    // `write_with_nonblocking` defers to the file description's `O_NONBLOCK`.
    let written = match write_from_iovec(&file, &iovecs, false) {
        Ok(written) => written,
        Err(FileError::BrokenPipe) => {
            let _ =
                roxy_process::send_signal(roxy_process::current_process_id(), Signal::BrokenPipe);
            return Err(Errno::Pipe);
        }
        Err(error) => return Err(map_file_error(error)),
    };

    Ok(u64::try_from(written).unwrap())
}

fn map_process_error(_: DescriptorError) -> Errno {
    Errno::BadFd
}
