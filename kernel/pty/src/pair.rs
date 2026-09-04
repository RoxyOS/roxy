use alloc::{collections::VecDeque, sync::Arc};

use roxy_arch::{Architecture, CurrentArchitectureBackend};
use roxy_fd::{FileError, FileMetadata, FileType, IoctlError, IoctlRequest, PollEvents};
use roxy_poll::{PollListener, PollListeners, PollRegistration};
use roxy_tty_core::{OutputError, TerminalInputSource, TtyCore, TtyOutput};
use roxy_tty_types::WindowSize;
use roxy_utils::Lock;

const MASTER_FILE_ID_BASE: u64 = 1000;
const SLAVE_FILE_ID_BASE: u64 = 2000;

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
pub struct PtyPair {
    number: u32,
    master_output: Arc<MasterOutput>,
    slave_input: Arc<SlaveInputSource>,
    slave_core: Arc<TtyCore>,
    locked: Lock<bool>,
}

impl PtyPair {
    pub(crate) fn new(number: u32) -> Arc<Self> {
        let master_output = Arc::new(MasterOutput {
            queue: Lock::new(VecDeque::new()),
            poll: Arc::new(PollListeners::new()),
        });
        let slave_input = Arc::new(SlaveInputSource {
            queue: Lock::new(VecDeque::new()),
        });
        let slave_core = TtyCore::new(master_output.clone(), slave_input.clone());

        Arc::new(Self {
            number,
            master_output,
            slave_input,
            slave_core,
            locked: Lock::new(true),
        })
    }
}

/// The master side of a pty pair, exposed through the fd opened from `/dev/ptmx`.
///
/// TODO(master-close-hangup): closing the last master does not yet signal EOF or `SIGHUP` to the
/// slave, because `Device` has no per-open drop hook to detect it.
pub struct PtyMaster {
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

impl roxy_devfs::Device for PtyMaster {
    fn metadata(&self) -> FileMetadata {
        FileMetadata {
            file_id: MASTER_FILE_ID_BASE + u64::from(self.pair.number),
            file_type: FileType::CharacterDevice,
            permissions: 0o600,
            size: 0,
            hard_links: 1,
        }
    }

    fn is_terminal(&self) -> bool {
        // A pty master is a "dumb" bidirectional pipe, not a terminal.
        false
    }

    fn read(&self, output: &mut [u8]) -> Result<usize, FileError> {
        if output.is_empty() {
            return Ok(0);
        }

        loop {
            let count = self.drain_master(output);
            if count > 0 {
                return Ok(count);
            }

            if roxy_process::has_pending_signal() {
                return Err(FileError::Interrupted);
            }

            assert!(!CurrentArchitectureBackend::interrupts_enabled());
            CurrentArchitectureBackend::wait_for_interrupt();
        }
    }

    fn write(&self, input: &[u8]) -> Result<usize, FileError> {
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

    fn poll(&self) -> PollEvents {
        PollEvents {
            readable: !self.pair.master_output.queue.lock().is_empty(),
            writable: true,
            ..PollEvents::default()
        }
    }

    fn register_poll_listener(&self, listener: Arc<PollListener>) -> PollRegistration {
        self.pair.master_output.poll.register(listener)
    }

    fn ioctl(&self, request: IoctlRequest<'_>) -> Result<(), IoctlError> {
        match request {
            IoctlRequest::PtyGetNumber(number) => {
                *number = self.pair.number;
                Ok(())
            }
            IoctlRequest::PtySetLock(locked) => {
                // TODO(pty-lock): the flag is recorded but slave `open` does not yet reject a
                // locked slave.
                *self.pair.locked.lock() = locked;
                Ok(())
            }
            _ => {
                // TODO(pty-gptpeer): `TIOCGPTPEER` is unsupported because the syscall layer cannot
                // return a newly allocated descriptor from ioctl; callers open `/dev/pts/N`.
                Err(IoctlError::NotTty)
            }
        }
    }
}

/// The slave side of a pty pair, exposed through `/dev/pts/N` and the program's controlling
/// terminal.
pub struct PtySlave {
    pair: Arc<PtyPair>,
}

impl PtySlave {
    pub(crate) fn new(pair: Arc<PtyPair>) -> Self {
        Self { pair }
    }
}

impl roxy_devfs::Device for PtySlave {
    fn metadata(&self) -> FileMetadata {
        FileMetadata {
            file_id: SLAVE_FILE_ID_BASE + u64::from(self.pair.number),
            file_type: FileType::CharacterDevice,
            permissions: 0o600,
            size: 0,
            hard_links: 1,
        }
    }

    fn is_terminal(&self) -> bool {
        true
    }

    fn read(&self, output: &mut [u8]) -> Result<usize, FileError> {
        self.pair.slave_core.read(output)
    }

    fn write(&self, input: &[u8]) -> Result<usize, FileError> {
        self.pair.slave_core.write(input)
    }

    fn poll(&self) -> PollEvents {
        self.pair.slave_core.poll().unwrap_or_default()
    }

    fn register_poll_listener(&self, listener: Arc<PollListener>) -> PollRegistration {
        self.pair.slave_core.register_poll_listener(listener)
    }

    fn ioctl(&self, request: IoctlRequest<'_>) -> Result<(), IoctlError> {
        self.pair.slave_core.ioctl(request)
    }
}
