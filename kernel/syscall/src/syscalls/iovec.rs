use alloc::vec::Vec;

use roxy_fd::{FileError, OpenFile};
use roxy_memory::UserAddress;

use crate::args::user_memory;
use crate::errno::Errno;

/// ABI-compatible `struct iovec` for `x86_64` little-endian.
///
/// Layout (per mlibc sysdeps/roxy/include/abi-bits/): { pointer, `size_t` } = 16 bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Iovec {
    pub(crate) base: UserAddress,
    pub(crate) length: usize,
}

const _: () = assert!(core::mem::size_of::<Iovec>() == 16);

/// Reads the `iovec` array at `address` (count = `count`) from user space.
///
/// Returns an empty vector when `count` is zero or negative.
///
/// # Errors
///
/// Returns `Fault` when the iovec array cannot be read.
pub(crate) fn read_iovecs(address: UserAddress, count: i32) -> Result<Vec<Iovec>, Errno> {
    if count <= 0 {
        return Ok(Vec::new());
    }

    #[allow(clippy::cast_sign_loss)]
    let count = count as usize;

    let mut iovecs = Vec::<Iovec>::new();
    iovecs.try_reserve_exact(count).map_err(|_| Errno::NoMem)?;
    iovecs.resize(
        count,
        Iovec {
            base: UserAddress::sentinel(),
            length: 0,
        },
    );

    // SAFETY: Iovec is repr(C) with integer/pointer fields that accept every bit pattern.
    unsafe { user_memory::read_slice(address, &mut iovecs) }?;

    Ok(iovecs)
}

/// Maximum bytes to read from user space in one pass.
const SCRATCH_SIZE: usize = 4096;

/// Writes data gathered from `iovecs` through `file`, honoring `nonblocking`.
///
/// When `nonblocking` is `false`, the file's own `O_NONBLOCK` flag governs blocking behaviour.
/// Returns the total number of bytes written.
///
/// # Errors
///
/// Returns `FileError::Io` when a user buffer cannot be read, plus any error from the write.
pub(crate) fn write_from_iovec(
    file: &OpenFile,
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

/// Maps a `FileError` to its ABI errno value.
pub(crate) fn map_file_error(error: FileError) -> Errno {
    match error {
        FileError::WouldBlock => Errno::Again,
        FileError::BadOperation => Errno::BadFd,
        FileError::BrokenPipe => Errno::Pipe,
        FileError::NotConnected => Errno::NotConnected,
        FileError::Io => Errno::Io,
        FileError::Interrupted => Errno::Interrupted,
    }
}
