use alloc::vec::Vec;

use roxy_fd::Fd;
use roxy_memory::UserAddress;

use crate::{SyscallResult, args::Slice, errno::Errno, numbers::SyscallNumber, syscall};

syscall!(
    SyscallNumber::Ttyname,
    handle(fd: Fd => BadFd, output: UserAddress => Fault, size: usize => Range)
);

/// Writes the device pathname of the terminal backing `fd` into `output`.
///
/// The path is owned by the terminal object itself (`File::terminal_path`), so this resolves
/// per terminal: the console answers `/dev/tty0`, a pty slave answers `/dev/pts/N`, and anything
/// that is not a terminal fails with `ENOTTY`.
fn handle(fd: Fd, output: UserAddress, size: usize) -> SyscallResult {
    let file = roxy_process::current_open_file(fd).map_err(map_process_error)?;
    let Some(path) = file.terminal_path() else {
        return Err(Errno::NotTty);
    };

    let encoded = encode(&path, size)?;
    let output = Slice::<u8>::new(output, size);

    // SAFETY: u8 has no padding and encoded contains only initialized bytes.
    unsafe { output.write(&encoded) }?;

    Ok(0)
}

/// NUL-terminates `name` into `size` bytes, failing when the buffer cannot hold the terminator.
fn encode(name: &[u8], size: usize) -> Result<Vec<u8>, Errno> {
    let required_size = name.len().checked_add(1).ok_or(Errno::Overflow)?;

    if size < required_size {
        return Err(Errno::Range);
    }

    let mut encoded = Vec::with_capacity(required_size);
    encoded.extend_from_slice(name);
    encoded.push(0);

    Ok(encoded)
}

fn map_process_error(_: roxy_process::DescriptorError) -> Errno {
    Errno::BadFd
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_test::kernel_test;

    use super::encode;
    use crate::errno::Errno;

    kernel_test!(
        "roxy-syscall::ttyname-encoding",
        encodes_null_terminated_name,
        {
            assert_eq!(encode(b"/dev/tty0", 10).unwrap(), b"/dev/tty0\0");
            // Exactly the required size is accepted.
            assert_eq!(encode(b"/dev/pts/3", 10).unwrap().len(), 10);
            // One byte short of the terminator is an error.
            assert_eq!(encode(b"/dev/tty0", 9), Err(Errno::Range));
        }
    );
}
