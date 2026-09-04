use alloc::{sync::Arc, vec::Vec};

use roxy_arch::{Architecture, CurrentArchitectureBackend};
use roxy_key_decoder::KeyDecoder;
use roxy_keyboard_input::KeyEvent;
use roxy_terminal::TerminalOutput;
use roxy_tty_core::{TerminalInputSource, TtyCore};
use roxy_tty_types::WindowSize;
use roxy_utils::Lock;

use crate::encoder::encode_decoded;

pub(crate) const PENDING_CAPACITY: usize = 256;

/// The console terminal's openable device path, reported by `terminal_path` for `ttyname`. It must
/// match the devfs node `CONSOLE_NODE` is registered under, so a `ttyname` consumer can reopen it.
/// There is a single console, so the path is a crate constant rather than a per-terminal field.
pub(crate) const CONSOLE_PATH: &[u8] = b"/dev/tty0";

/// The mount-relative devfs node name the console is registered under (devfs is mounted at `/dev`,
/// so opening `/dev/tty0` resolves to leaf node `tty0` below it). This must be the node that
/// `CONSOLE_PATH` reopens.
pub(crate) const CONSOLE_NODE: &[u8] = b"tty0";

/// Turns raw keyboard key events into the byte stream consumed by the shared terminal core.
///
/// This is the console-specific side of the terminal: it owns the pending key queue, the US 104-key
/// layout decoder, and the encoding of decoded keys into UTF-8/escape-sequence bytes. All terminal
/// processing downstream of these bytes belongs to `TtyCore`.
pub(crate) struct ConsoleInputSource {
    pending: Lock<heapless::Deque<KeyEvent, PENDING_CAPACITY>>,
    decoder: Lock<KeyDecoder>,
}

impl ConsoleInputSource {
    fn new() -> Self {
        Self {
            pending: Lock::new(heapless::Deque::new()),
            decoder: Lock::new(KeyDecoder::new()),
        }
    }
}

impl TerminalInputSource for ConsoleInputSource {
    fn next_input_bytes(&self) -> Option<Vec<u8>> {
        loop {
            let event =
                CurrentArchitectureBackend::without_interrupts(|| self.pending.lock().pop_front())?;
            let decoded_key = self.decoder.lock().decode(event)?;
            if let Some(encoded) = encode_decoded(decoded_key) {
                return Some(encoded.as_bytes().to_vec());
            }
        }
    }

    fn try_peek_bytes(&self) -> Option<Vec<u8>> {
        // IRQ/callback path: try-lock the decoder first (matching the read path's decoder-before
        // line-discipline ordering pushed up into the core), then read the front key without
        // consuming it. The core only calls `consume_peeked` once it has locked the discipline.
        let mut decoder = self.decoder.try_lock()?;
        let event = *self.pending.lock().front()?;
        let decoded_key = decoder.decode(event)?;
        encode_decoded(decoded_key).map(|encoded| encoded.as_bytes().to_vec())
    }

    fn consume_peeked(&self) {
        let _ = self.pending.lock().pop_front();
    }

    fn discard_pending_input(&self) {
        // The IRQ path pushes into this queue; draining must disable interrupts so it cannot run
        // while this thread holds the queue lock.
        CurrentArchitectureBackend::without_interrupts(|| {
            while self.pending.lock().pop_front().is_some() {}
        });
    }
}

/// Adapters a `roxy-terminal::TerminalOutput` endpoint into the core's `TtyOutput` contract.
struct OutputAdapter {
    output: Arc<dyn TerminalOutput>,
}

impl roxy_tty_core::TtyOutput for OutputAdapter {
    fn write(&self, input: &[u8]) -> Result<usize, roxy_tty_core::OutputError> {
        self.output
            .write(input)
            .map_err(|_| roxy_tty_core::OutputError::Io)
    }

    fn window_size(&self) -> WindowSize {
        self.output.window_size()
    }
}

/// The console terminal: a keyboard-driven user of the shared terminal core.
///
/// Holds the keyboard input source and delegates all terminal behavior (line discipline, buffering,
/// blocking reads, ioctls, foreground group and session semantics) to `TtyCore`.
pub struct Tty {
    core: Arc<TtyCore>,
    input: Arc<ConsoleInputSource>,
}

impl Tty {
    pub(super) fn new(output: Arc<dyn TerminalOutput>) -> Self {
        let input = Arc::new(ConsoleInputSource::new());
        let core = TtyCore::new(Arc::new(OutputAdapter { output }), input.clone());

        Self { core, input }
    }

    /// Queues one raw key event (test/IO path used by the keyboard manager and tests).
    pub(crate) fn push_key(&self, key: KeyEvent) {
        let _ = self.input.pending.lock().push_back(key);
    }

    /// IRQ/callback-safe fast path that delivers VINTR/SIGINT immediately.
    pub(crate) fn try_process_input_arrival(&self) {
        self.core.try_process_input_arrival();
    }

    /// Wakes any reader blocked in the core.
    pub(crate) fn observe_input(&self) {
        self.core.observe_input();
    }

    pub(crate) fn bind_session(
        &self,
        session: roxy_process::SessionId,
        pgid: roxy_process::ProcessGroupId,
    ) {
        self.core.bind_session(session, pgid);
    }

    pub(super) fn read(&self, output: &mut [u8]) -> Result<usize, roxy_fd::FileError> {
        self.core.read(output)
    }

    pub(super) fn write(&self, input: &[u8]) -> Result<usize, roxy_fd::FileError> {
        self.core.write(input)
    }

    pub(super) fn poll(&self) -> Result<roxy_fd::PollEvents, roxy_fd::FileError> {
        self.core.poll()
    }

    pub(super) fn register_poll_listener(
        &self,
        listener: Arc<roxy_poll::PollListener>,
    ) -> roxy_poll::PollRegistration {
        self.core.register_poll_listener(listener)
    }

    pub(super) fn ioctl(
        &self,
        request: roxy_fd::IoctlRequest<'_>,
    ) -> Result<(), roxy_fd::IoctlError> {
        self.core.ioctl(request)
    }

    /// Returns the console's openable device path for `ttyname`.
    pub(super) fn terminal_path() -> &'static [u8] {
        CONSOLE_PATH
    }
}
