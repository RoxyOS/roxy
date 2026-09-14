use roxy_fd::{Fd, SeekError, SeekFrom};
use roxy_process::DescriptorError;

use crate::{SyscallResult, args::SyscallArg, errno::Errno, numbers::SyscallNumber, syscall};

syscall!(SyscallNumber::Seek, handle(fd: Fd => BadFd, offset: i64, whence: SeekWhence => Invalid));

/// Roxy numbers `whence` from a base above Linux's range, so a Linux-valued `whence` is
/// reported as a foreign numbering instead of being silently honoured.
const SEEK_BASE: u64 = 1 << 8;

const SEEK_SET: u64 = SEEK_BASE;
const SEEK_CURRENT: u64 = SEEK_BASE + 1;
const SEEK_END: u64 = SEEK_BASE + 2;

#[repr(u64)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SeekWhence {
    Set = SEEK_SET,
    Current = SEEK_CURRENT,
    End = SEEK_END,
}

impl SeekWhence {
    fn position(self, offset: i64) -> Result<SeekFrom, Errno> {
        match self {
            Self::Set => u64::try_from(offset)
                .map(SeekFrom::Start)
                .map_err(|_| Errno::Invalid),
            Self::Current => Ok(SeekFrom::Current(offset)),
            Self::End => Ok(SeekFrom::End(offset)),
        }
    }
}

impl SyscallArg for SeekWhence {
    fn parse(raw: u64, _error: Errno) -> Result<Self, Errno> {
        match raw {
            SEEK_SET => Ok(Self::Set),
            SEEK_CURRENT => Ok(Self::Current),
            SEEK_END => Ok(Self::End),
            value if value < SEEK_BASE => Err(crate::unsupported::unsupported_argument(
                "seek.whence.foreign",
                value,
                Errno::Invalid,
            )),
            value => Err(crate::unsupported::unsupported_argument(
                "seek.whence",
                value,
                Errno::Invalid,
            )),
        }
    }
}

fn handle(fd: Fd, offset: i64, whence: SeekWhence) -> SyscallResult {
    let file = roxy_process::current_open_file(fd).map_err(map_process_error)?;
    let position = whence.position(offset)?;

    let new_offset = file.seek(position).map_err(map_seek_error)?;
    let new_offset = i64::try_from(new_offset).map_err(|_| Errno::Overflow)?;

    Ok(new_offset.cast_unsigned())
}

fn map_process_error(_: DescriptorError) -> Errno {
    Errno::BadFd
}

fn map_seek_error(error: SeekError) -> Errno {
    match error {
        SeekError::NotSeekable => Errno::Pipe,
        SeekError::InvalidOffset => Errno::Invalid,
        SeekError::Overflow => Errno::Overflow,
        SeekError::Io => Errno::Io,
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_fd::SeekFrom;
    use roxy_test::kernel_test;

    use super::{SEEK_CURRENT, SEEK_END, SEEK_SET, SeekWhence};
    use crate::{args::SyscallArg, errno::Errno};

    kernel_test!("roxy-syscall::seek-positions", parses_standard_positions, {
        assert_eq!(
            SeekWhence::parse(SEEK_SET, Errno::Invalid)
                .unwrap()
                .position(7),
            Ok(SeekFrom::Start(7))
        );
        assert_eq!(
            SeekWhence::parse(SEEK_CURRENT, Errno::Invalid)
                .unwrap()
                .position(-2),
            Ok(SeekFrom::Current(-2))
        );
        assert_eq!(
            SeekWhence::parse(SEEK_END, Errno::Invalid)
                .unwrap()
                .position(3),
            Ok(SeekFrom::End(3))
        );
        assert_eq!(
            SeekWhence::parse(SEEK_SET, Errno::Invalid)
                .unwrap()
                .position(-1),
            Err(Errno::Invalid)
        );
    });

    kernel_test!(
        "roxy-syscall::seek-whence",
        separates_foreign_from_undefined,
        {
            // Linux numbers `SEEK_SET`, `SEEK_CUR`, and `SEEK_END` from zero, so every one of them
            // is below the Roxy base and is another personality's numbering.
            assert_eq!(SeekWhence::parse(0, Errno::Invalid), Err(Errno::Invalid));
            assert_eq!(SeekWhence::parse(1, Errno::Invalid), Err(Errno::Invalid));
            assert_eq!(SeekWhence::parse(2, Errno::Invalid), Err(Errno::Invalid));

            // A value above the base that no position names is a request of ours.
            assert_eq!(
                SeekWhence::parse(SEEK_END + 1, Errno::Invalid),
                Err(Errno::Invalid)
            );
        }
    );
}
