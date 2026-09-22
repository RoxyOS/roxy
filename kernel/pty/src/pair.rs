use alloc::{collections::VecDeque, sync::Arc};
use core::sync::atomic::{AtomicU32, Ordering};

use roxy_arch::{Architecture, CurrentArchitectureBackend};
use roxy_fd::{
    File, FileError, FileMetadata, FileType, IoctlError, IoctlRequest, PollEvents, SeekError,
    SeekFrom,
};
use roxy_poll::{PollListener, PollListeners, PollRegistration};
use roxy_tty_core::{OutputError, TerminalInputSource, TtyCore, TtyOutput};
use roxy_tty_types::WindowSize;
use roxy_utils::Lock;

const MASTER_FILE_ID_BASE: u64 = 1000;
const SLAVE_FILE_ID_BASE: u64 = 2000;

/// The file number of the next pair, unique for the kernel lifetime.
static NEXT_NUMBER: AtomicU32 = AtomicU32::new(0);

/// The slave's output destination: the pty master's receive buffer.
///
/// This is how the slave's write and its echo reach the master reader. A write wakes any master
/// reader blocked in `PtyMaster::read`.
struct MasterOutput {
    queue: Lock<VecDeque<u8>>,
    poll: Arc<PollListeners>,
}

impl TtyOutput for MasterOutput {
    fn write(&self, input: &[u8]) -> Result<usize, OutputError> {
        self.queue.lock().extend(input.iter().copied());
        self.poll.notify();

        Ok(input.len())
    }

    fn window_size(&self) -> WindowSize {
        WindowSize::UNKNOWN
    }
}

/// The pty master's keystroke stream, fed into the slave's line discipline.
///
/// It yields the stream one byte at a time so a newline reaches the discipline as its own event
/// and canonical mode can commit it. The master's `write` extends this queue and wakes the slave's
/// reader via the core.
struct SlaveInputSource {
    queue: Lock<VecDeque<u8>>,
}

impl TerminalInputSource for SlaveInputSource {
    fn next_input_bytes(&self) -> Option<alloc::vec::Vec<u8>> {
        Some(alloc::vec![self.queue.lock().pop_front()?])
    }

    fn try_peek_bytes(&self) -> Option<alloc::vec::Vec<u8>> {
        Some(alloc::vec![*self.queue.lock().front()?])
    }

    fn consume_peeked(&self) {
        self.queue.lock().pop_front();
    }

    fn discard_pending_input(&self) {
        self.queue.lock().clear();
    }
}

/// One pseudo-terminal pair: a master the terminal emulator holds and a slave that is the
/// controlling terminal of the program running inside it.
///
/// A pair is allocated only by [`crate::open_pair`]; it has no device-filesystem name, and the
/// slave is reachable solely through the descriptor `openpty` returns.
pub(crate) struct PtyPair {
    number: u32,
    master_output: Arc<MasterOutput>,
    slave_input: Arc<SlaveInputSource>,
    slave_core: Arc<TtyCore>,
}

impl PtyPair {
    pub(crate) fn new() -> Arc<Self> {
        let number = NEXT_NUMBER.fetch_add(1, Ordering::Relaxed);
        let master_output = Arc::new(MasterOutput {
            queue: Lock::new(VecDeque::new()),
            poll: Arc::new(PollListeners::new()),
        });
        let slave_input = Arc::new(SlaveInputSource {
            queue: Lock::new(VecDeque::new()),
        });
        let slave_core = TtyCore::new(master_output.clone(), slave_input.clone(), None);

        Arc::new(Self {
            number,
            master_output,
            slave_input,
            slave_core,
        })
    }
}

/// The master side of a pty pair: a "dumb" bidirectional pipe with no line discipline.
///
/// TODO(master-close-hangup): closing the last master does not yet signal EOF or `SIGHUP` to the
/// slave, because the descriptor layer has no per-open drop hook to detect it.
pub(crate) struct PtyMaster {
    pair: Arc<PtyPair>,
}

impl PtyMaster {
    pub(crate) fn new(pair: Arc<PtyPair>) -> Self {
        Self { pair }
    }

    fn drain_master(&self, output: &mut [u8]) -> usize {
        let mut queue = self.pair.master_output.queue.lock();
        let count = output.len().min(queue.len());

        for byte in &mut output[..count] {
            *byte = queue.pop_front().expect("count bounded by queue length");
        }

        count
    }
}

impl File for PtyMaster {
    fn poll(&mut self) -> Result<PollEvents, FileError> {
        Ok(PollEvents {
            readable: !self.pair.master_output.queue.lock().is_empty(),
            writable: true,
            ..PollEvents::default()
        })
    }

    fn register_poll_listener(&mut self, listener: Arc<PollListener>) -> PollRegistration {
        self.pair.master_output.poll.register(listener)
    }

    fn is_terminal(&self) -> bool {
        false
    }

    fn metadata(&self) -> Result<FileMetadata, FileError> {
        Ok(FileMetadata {
            file_id: MASTER_FILE_ID_BASE + u64::from(self.pair.number),
            file_type: FileType::CharacterDevice,
            permissions: 0o600,
            size: 0,
            hard_links: 1,
        })
    }

    fn read(
        &mut self,
        _position: &mut u64,
        output: &mut [u8],
        nonblocking: bool,
    ) -> Result<usize, FileError> {
        if output.is_empty() {
            return Ok(0);
        }

        loop {
            let count = self.drain_master(output);
            if count > 0 {
                return Ok(count);
            }

            if nonblocking {
                return Err(FileError::WouldBlock);
            }

            if roxy_process::has_pending_signal() {
                return Err(FileError::Interrupted);
            }

            assert!(!CurrentArchitectureBackend::interrupts_enabled());
            CurrentArchitectureBackend::wait_for_interrupt();
        }
    }

    fn write(
        &mut self,
        _position: &mut u64,
        input: &[u8],
        _nonblocking: bool,
    ) -> Result<usize, FileError> {
        // Feed the slave's line discipline as if the user typed these bytes.
        self.pair
            .slave_input
            .queue
            .lock()
            .extend(input.iter().copied());
        self.pair.slave_core.try_process_input_arrival();
        self.pair.slave_core.observe_input();

        Ok(input.len())
    }

    fn seek(&mut self, _current: u64, _position: SeekFrom) -> Result<u64, SeekError> {
        Err(SeekError::NotSeekable)
    }

    fn ioctl(&mut self, _request: IoctlRequest<'_>) -> Result<(), IoctlError> {
        // The master carries no termios of its own; `openpty` applies terminal attributes to the
        // slave descriptor.
        Err(IoctlError::NotTty)
    }
}

/// The slave side of a pty pair: the program's controlling terminal.
///
/// Every operation delegates to the pair's `TtyCore`, so the slave inherits line discipline,
/// canonical editing, termios, foreground groups, and controlling-session handling.
pub(crate) struct PtySlave {
    pair: Arc<PtyPair>,
}

impl PtySlave {
    pub(crate) fn new(pair: Arc<PtyPair>) -> Self {
        Self { pair }
    }
}

impl File for PtySlave {
    fn poll(&mut self) -> Result<PollEvents, FileError> {
        self.pair.slave_core.poll()
    }

    fn register_poll_listener(&mut self, listener: Arc<PollListener>) -> PollRegistration {
        self.pair.slave_core.register_poll_listener(listener)
    }

    fn is_terminal(&self) -> bool {
        true
    }

    fn metadata(&self) -> Result<FileMetadata, FileError> {
        Ok(FileMetadata {
            file_id: SLAVE_FILE_ID_BASE + u64::from(self.pair.number),
            file_type: FileType::CharacterDevice,
            permissions: 0o600,
            size: 0,
            hard_links: 1,
        })
    }

    fn read(
        &mut self,
        _position: &mut u64,
        output: &mut [u8],
        _nonblocking: bool,
    ) -> Result<usize, FileError> {
        self.pair.slave_core.read(output)
    }

    fn write(
        &mut self,
        _position: &mut u64,
        input: &[u8],
        _nonblocking: bool,
    ) -> Result<usize, FileError> {
        self.pair.slave_core.write(input)
    }

    fn seek(&mut self, _current: u64, _position: SeekFrom) -> Result<u64, SeekError> {
        Err(SeekError::NotSeekable)
    }

    fn ioctl(&mut self, request: IoctlRequest<'_>) -> Result<(), IoctlError> {
        self.pair.slave_core.ioctl(request)
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_fd::FileError;
    use roxy_test::kernel_test;

    use crate::open_pair;

    kernel_test!(
        "roxy-pty::open-pair",
        gives_master_and_slave_distinct_files,
        {
            let (master, slave) = open_pair();

            assert!(!master.is_terminal());
            assert!(slave.is_terminal());

            let master_id = master.metadata().unwrap().file_id;
            let slave_id = slave.metadata().unwrap().file_id;
            assert_ne!(master_id, slave_id);
        }
    );

    kernel_test!("roxy-pty::open-pair-numbers", numbers_pairs_uniquely, {
        let (first, _) = open_pair();
        let (second, _) = open_pair();

        assert_ne!(
            first.metadata().unwrap().file_id,
            second.metadata().unwrap().file_id
        );
    });

    kernel_test!("roxy-pty::slave-has-no-path", slave_has_no_device_name, {
        let (_, slave) = open_pair();

        assert!(slave.is_terminal());
    });

    kernel_test!(
        "roxy-pty::master-to-slave",
        delivers_master_input_to_the_slave,
        {
            let (master, slave) = open_pair();

            assert_eq!(master.write(b"hi\n"), Ok(3));

            // Canonical mode commits the line at the newline, so the read does not block.
            let mut input = [0u8; 8];
            let count = slave.read(&mut input).unwrap();
            assert_eq!(&input[..count], b"hi\n");
        }
    );

    kernel_test!(
        "roxy-pty::slave-to-master",
        delivers_slave_output_to_the_master,
        {
            let (master, slave) = open_pair();

            assert_eq!(slave.write(b"out"), Ok(3));

            let mut output = [0u8; 8];
            let count = master.read_with_nonblocking(&mut output, true).unwrap();
            assert_eq!(&output[..count], b"out");
        }
    );

    kernel_test!(
        "roxy-pty::master-nonblocking",
        reports_would_block_without_data,
        {
            let (master, _slave) = open_pair();

            let mut output = [0u8; 8];
            assert_eq!(
                master.read_with_nonblocking(&mut output, true),
                Err(FileError::WouldBlock)
            );
        }
    );
}
