use roxy_fd::{Fd, SeekError, SeekFrom};
use roxy_process::DescriptorError;

use crate::{Syscall, SyscallResult, errno::Errno, numbers::SyscallNumber};

pub(super) const SYSCALL: Syscall = Syscall::new(SyscallNumber::Seek, handle);

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
    fn parse(value: u64) -> Result<Self, Errno> {
        match value {
            0 => Ok(Self::Set),
            1 => Ok(Self::Current),
            2 => Ok(Self::End),
            3 => Ok(Self::Data),
            4 => Ok(Self::Hole),
            _ => Err(Errno::Invalid),
        }
    }

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

fn handle(arguments: [u64; 6]) -> SyscallResult {
    let fd = u32::try_from(arguments[0])
        .map(Fd::new)
        .map_err(|_| Errno::BadFd)?;
    let offset = arguments[1].cast_signed();
    let whence = SeekWhence::parse(arguments[2])?;

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

#[cfg(test)]
mod tests {
    use roxy_fd::SeekFrom;

    use super::SeekWhence;
    use crate::errno::Errno;

    #[test]
    fn parses_standard_seek_positions() {
        assert_eq!(
            SeekWhence::parse(0).unwrap().position(7),
            Ok(SeekFrom::Start(7))
        );
        assert_eq!(
            SeekWhence::parse(1).unwrap().position(-2),
            Ok(SeekFrom::Current(-2))
        );
        assert_eq!(
            SeekWhence::parse(2).unwrap().position(3),
            Ok(SeekFrom::End(3))
        );
        assert_eq!(
            SeekWhence::parse(0).unwrap().position(-1),
            Err(Errno::Invalid)
        );
        assert_eq!(SeekWhence::parse(5), Err(Errno::Invalid));
    }
}
