#![no_std]

extern crate alloc;

mod encoder;
mod file;
mod ioctl;
mod tty;

use alloc::{sync::Arc, vec::Vec};

use heapless::Deque;
use roxy_fd::OpenFile;
use roxy_key_decoder::KeyDecoder;
use roxy_keyboard_input::{KeyEvent, KeyboardListener};
use roxy_line_discipline::LineDiscipline;
use roxy_poll::PollListeners;
use roxy_process::ProcessGroupId;
use roxy_process::SessionId;
use roxy_terminal::TerminalOutput;
use roxy_tty_types::WindowSize;
use roxy_utils::Lock;
use spin::Once;

use file::TtyFile;

const PENDING_CAPACITY: usize = 256;

struct Tty {
    output: Arc<dyn TerminalOutput>,
    line_discipline: Lock<LineDiscipline>,
    /// Decodes raw key events into characters and special keys
    /// (US 104-key layout, `MapLettersToUnicode`).
    decoder: Lock<KeyDecoder>,
    window_size: Lock<WindowSize>,
    buffered: Lock<Vec<u8>>,
    /// Bounded queue of raw key events received from the input manager.
    ///
    /// The IRQ path pushes events here and tries to process them immediately (with `try_lock`
    /// on decoder and line discipline). The read path pops events from here when the buffer is
    /// empty. The queue is sized to match the previous PS/2 driver queue depth.
    pending: Lock<Deque<KeyEvent, PENDING_CAPACITY>>,
    read_lock: Lock<()>,
    poll_listeners: Arc<PollListeners>,
    /// The process group that receives terminal-generated signals, when one is selected.
    foreground_pgid: Lock<Option<ProcessGroupId>>,
    /// The session that owns this controlling terminal, when one is established.
    owner_session_id: Lock<Option<SessionId>>,
}

static TTY: Once<Arc<Tty>> = Once::new();

/// Publishes the one TTY used for initial process descriptors.
///
/// Returns an `KeyboardListener` that main registers with the process-wide input manager.
///
/// # Panics
///
/// Panics when called more than once.
pub fn initialize(output: Arc<dyn TerminalOutput>) -> Arc<dyn KeyboardListener> {
    assert!(TTY.get().is_none(), "TTY initialized twice");
    let tty = Arc::new(Tty::new(output));
    TTY.call_once(|| tty.clone());
    roxy_process::register_session_leader_exit_handler(on_session_leader_exit);
    tty
}

/// Sends SIGHUP to the foreground process group of this terminal when its controlling session's
/// leader exits, then releases the terminal (Linux `disassociate_ctty`/hangup semantics).
///
/// Runs on the exiting session leader's own thread via the process-side callback, so no
/// process-table lock is held; the locks taken here are the terminal's own.
fn on_session_leader_exit(session: roxy_process::SessionId) {
    let Some(tty) = TTY.get() else {
        return;
    };

    let owns_terminal = *tty.owner_session_id.lock() == Some(session);
    if !owns_terminal {
        return;
    }

    let foreground = tty.foreground_pgid.lock().take();
    *tty.owner_session_id.lock() = None;

    if let Some(pgid) = foreground {
        roxy_process::send_signal_to_pgid(pgid, roxy_signal::Signal::Hangup);
    }
}

impl KeyboardListener for Tty {
    fn on_recive_input(&self, key: KeyEvent) {
        // IRQ context: interrupts are already disabled, so a plain lock on the pending queue
        // is safe (the read path disables interrupts while holding it).
        let _ = self.pending.lock().push_back(key);
        self.try_process_input_arrival();
        self.poll_listeners.notify();
    }
}

/// Opens an independent descriptor for the initialized TTY.
///
/// # Panics
///
/// Panics when the TTY has not been initialized.
#[must_use]
pub fn open() -> Arc<OpenFile> {
    let tty = TTY.get().expect("TTY must be initialized before opening");

    TtyFile::open(tty.clone())
}

/// Binds the terminal to a session as its controlling terminal, setting the session's initial
/// foreground process group.
///
/// This is how the composition root establishes the initial controlling terminal in the role
/// that a login/getty program plays on Linux: the terminal is acquired by a fresh session leader
/// (the spawned init shell) and its foreground group is the leader's group.
///
/// # Panics
///
/// Panics when the TTY has not been initialized or is already bound to a session.
pub fn bind_controlling_terminal(session: SessionId, pgid: ProcessGroupId) {
    let tty = TTY.get().expect("TTY must be initialized before binding");
    let mut current_session = tty.owner_session_id.lock();

    assert!(
        current_session.is_none(),
        "TTY controlling terminal bound twice"
    );
    *current_session = Some(session);
    *tty.foreground_pgid.lock() = Some(pgid);
}

#[cfg(feature = "kernel-test")]
mod test_support {
    use alloc::{sync::Arc, vec::Vec};
    use core::sync::atomic::{AtomicUsize, Ordering};

    use roxy_fd::OpenFile;
    use roxy_keyboard_input::{KeyCode, KeyEvent, KeyState};
    use roxy_terminal::{OutputError, TerminalOutput};
    use roxy_tty_types::WindowSize;
    use spin::Mutex;

    use crate::Tty;
    use crate::file::TtyFile;

    pub(super) struct MockOutput {
        bytes: Mutex<Vec<u8>>,
        window_size: WindowSize,
        calls: AtomicUsize,
        fail_call: AtomicUsize,
        max_write: AtomicUsize,
    }

    impl MockOutput {
        fn new() -> Self {
            Self {
                bytes: Mutex::new(Vec::new()),
                window_size: WindowSize {
                    rows: 30,
                    columns: 100,
                    pixel_width: 800,
                    pixel_height: 480,
                },
                calls: AtomicUsize::new(0),
                fail_call: AtomicUsize::new(0),
                max_write: AtomicUsize::new(usize::MAX),
            }
        }

        pub(super) fn bytes(&self) -> Vec<u8> {
            self.bytes.lock().clone()
        }

        pub(super) fn fail_on_call(&self, call: usize) {
            self.fail_call.store(call, Ordering::Relaxed);
        }

        pub(super) fn limit_writes(&self, count: usize) {
            self.max_write.store(count, Ordering::Relaxed);
        }
    }

    impl TerminalOutput for MockOutput {
        fn write(&self, input: &[u8]) -> Result<usize, OutputError> {
            let call = self.calls.fetch_add(1, Ordering::Relaxed) + 1;

            if self.fail_call.load(Ordering::Relaxed) == call {
                return Err(OutputError::Io);
            }

            let count = input.len().min(self.max_write.load(Ordering::Relaxed));
            self.bytes.lock().extend_from_slice(&input[..count]);

            Ok(count)
        }

        fn window_size(&self) -> WindowSize {
            self.window_size
        }
    }

    /// Build a raw key event for test injection.
    pub(super) fn key(code: KeyCode, state: KeyState) -> KeyEvent {
        KeyEvent { code, state }
    }

    pub(super) fn open(events: Vec<KeyEvent>) -> (Arc<Tty>, Arc<MockOutput>, Arc<OpenFile>) {
        let output = Arc::new(MockOutput::new());
        let tty = Arc::new(Tty::new(output.clone()));
        // Inject events directly into the pending queue (no IRQ-path processing).
        for event in events {
            let _ = tty.pending.lock().push_back(event);
        }
        let file = TtyFile::open(tty.clone());

        (tty, output, file)
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_keyboard_input::{KeyCode, KeyEvent, KeyState};
    use roxy_test::kernel_test;
    use roxy_tty_types::WindowSize;

    use super::file::TtyFile;
    use super::test_support::{key, open};

    /// Helper that constructs a key-press event for test brevity.
    fn press(code: KeyCode) -> KeyEvent {
        key(code, KeyState::Pressed)
    }

    kernel_test!("roxy-tty::canonical-edit", edits_before_delivery, {
        let events = alloc::vec![
            press(KeyCode::A),
            press(KeyCode::B),
            press(KeyCode::Backspace),
            press(KeyCode::C),
            press(KeyCode::Return),
        ];
        let (_tty, output, file) = open(events);
        let mut buffer = [0; 8];

        // Type "ab", backspace erases 'b', then "c", newline commits "ac\n".
        assert_eq!(file.read(&mut buffer), Ok(3));
        assert_eq!(&buffer[..3], b"ac\n");
        assert_eq!(output.bytes(), b"ab\x08c\n");
    });

    kernel_test!("roxy-tty::canonical-escape", commits_escape_sequence, {
        let events = alloc::vec![press(KeyCode::ArrowLeft), press(KeyCode::Return),];
        let (_tty, output, file) = open(events);
        let mut buffer = [0; 4];

        assert_eq!(file.read(&mut buffer), Ok(4));
        assert_eq!(&buffer, b"\x1b[D\n");
        assert_eq!(output.bytes(), b"\x1b[D\n");
    });

    kernel_test!("roxy-tty::shared-line", shares_partial_canonical_line, {
        let (tty, output, first) = open(alloc::vec![
            press(KeyCode::A),
            press(KeyCode::B),
            press(KeyCode::C),
            press(KeyCode::Return),
        ]);
        let second = TtyFile::open(tty);
        let mut first_half = [0; 2];
        let mut second_half = [0; 2];

        assert_eq!(first.read(&mut first_half), Ok(2));
        assert_eq!(second.read(&mut second_half), Ok(2));
        assert_eq!(&first_half, b"ab");
        assert_eq!(&second_half, b"c\n");
        assert_eq!(output.bytes(), b"abc\n");
    });

    kernel_test!("roxy-tty::disabled-echo", skips_disabled_echo, {
        let (tty, output, file) = open(alloc::vec![press(KeyCode::X), press(KeyCode::Return),]);
        tty.line_discipline.lock().settings.echo = false;
        let mut buffer = [0; 2];

        assert_eq!(file.read(&mut buffer), Ok(2));
        assert_eq!(&buffer, b"x\n");
        assert!(output.bytes().is_empty());
    });

    kernel_test!("roxy-tty::initial-window-size", inherits_output_size, {
        let (tty, _output, _file) = open(alloc::vec![]);

        assert_eq!(
            *tty.window_size.lock(),
            WindowSize {
                rows: 30,
                columns: 100,
                pixel_width: 800,
                pixel_height: 480,
            }
        );
    });
}
