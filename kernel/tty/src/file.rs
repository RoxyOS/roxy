use alloc::{boxed::Box, sync::Arc};

use roxy_fd::{
    File, FileError, FileMetadata, FileType, IoctlError, IoctlRequest, OpenFile, PollEvents,
    SeekError, SeekFrom,
};

use crate::Tty;

pub(super) struct TtyFile {
    tty: Arc<Tty>,
}

impl File for TtyFile {
    fn is_terminal(&self) -> bool {
        true
    }

    fn metadata(&self) -> Result<FileMetadata, FileError> {
        Ok(FileMetadata {
            file_id: 1,
            file_type: FileType::CharacterDevice,
            permissions: 0o600,
            size: 0,
            hard_links: 1,
        })
    }

    fn read(&mut self, _position: &mut u64, output: &mut [u8]) -> Result<usize, FileError> {
        self.tty.read(output)
    }

    fn poll(&mut self) -> Result<PollEvents, FileError> {
        self.tty.poll()
    }

    fn write(&mut self, _position: &mut u64, input: &[u8]) -> Result<usize, FileError> {
        self.tty.write(input)
    }

    fn seek(&mut self, _current: u64, _position: SeekFrom) -> Result<u64, SeekError> {
        Err(SeekError::NotSeekable)
    }

    fn ioctl(&mut self, request: IoctlRequest<'_>) -> Result<(), IoctlError> {
        self.tty.ioctl(request)
    }
}

impl TtyFile {
    #[must_use]
    pub(super) fn open(tty: Arc<Tty>) -> Arc<OpenFile> {
        OpenFile::new(Box::new(Self { tty }))
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_fd::{FileType, SeekError, SeekFrom};
    use roxy_test::kernel_test;

    use crate::test_support::{character, open};

    kernel_test!("roxy-tty::file-adapter", delegates_tty_io, {
        let (_tty, output, file) = open(alloc::vec![character('x'), character('\n')]);
        let mut buffer = [0; 4];

        assert!(file.is_terminal());
        assert_eq!(
            file.metadata().unwrap().file_type,
            FileType::CharacterDevice
        );
        assert_eq!(file.metadata().unwrap().file_id, 1);
        assert_eq!(file.metadata().unwrap().permissions, 0o600);
        assert_eq!(file.read(&mut buffer), Ok(2));
        assert_eq!(&buffer[..2], b"x\n");
        assert_eq!(file.write(b"one"), Ok(3));
        assert_eq!(file.write(b"two"), Ok(3));
        assert_eq!(output.bytes(), b"x\nonetwo");
        assert_eq!(file.seek(SeekFrom::Start(0)), Err(SeekError::NotSeekable));
    });
}
