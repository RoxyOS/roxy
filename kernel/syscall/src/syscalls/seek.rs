use roxy_fd::{Fd, SeekError, SeekFrom};
use roxy_process::DescriptorError;

use crate::{SyscallResult, args::SyscallArg, errno::Errno, numbers::SyscallNumber, syscall};

syscall!(SyscallNumber::Seek, handle(fd: Fd => BadFd, offset: i64, whence: SeekWhence => Invalid));

#[repr(u64)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SeekWhence {
    Set = 0,
    Current = 1,
    End = 2,
    Data = 3,
    Hole = 4,
}

impl SeekWhence {
    fn position(self, offset: i64) -> Result<SeekFrom, Errno> {
        match self {
            Self::Set => u64::try_from(offset)
                .map(SeekFrom::Start)
                .map_err(|_| Errno::Invalid),
            Self::Current => Ok(SeekFrom::Current(offset)),
            Self::End => Ok(SeekFrom::End(offset)),
            Self::Data | Self::Hole => Err(crate::unsupported::unsupported_argument(
                "seek.whence",
                self as u64,
                Errno::NotSupported,
            )),
        }
    }
}

impl SyscallArg for SeekWhence {
    fn parse(raw: u64, error: Errno) -> Result<Self, Errno> {
        match raw {
            0 => Ok(Self::Set),
            1 => Ok(Self::Current),
            2 => Ok(Self::End),
            3 => Ok(Self::Data),
            4 => Ok(Self::Hole),
            _ => Err(error),
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

    use super::SeekWhence;
    use crate::{args::SyscallArg, errno::Errno};

    kernel_test!("roxy-syscall::seek-positions", parses_standard_positions, {
        assert_eq!(
            SeekWhence::parse(0, Errno::Invalid).unwrap().position(7),
            Ok(SeekFrom::Start(7))
        );
        assert_eq!(
            SeekWhence::parse(1, Errno::Invalid).unwrap().position(-2),
            Ok(SeekFrom::Current(-2))
        );
        assert_eq!(
            SeekWhence::parse(2, Errno::Invalid).unwrap().position(3),
            Ok(SeekFrom::End(3))
        );
        assert_eq!(
            SeekWhence::parse(0, Errno::Invalid).unwrap().position(-1),
            Err(Errno::Invalid)
        );
        assert_eq!(SeekWhence::parse(5, Errno::Invalid), Err(Errno::Invalid));
    });
}
