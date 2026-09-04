#![no_std]

extern crate alloc;

mod encoder;
mod file;
mod tty;

use alloc::sync::Arc;

use roxy_fd::OpenFile;
use roxy_keyboard_input::{KeyEvent, KeyboardListener};
use roxy_process::{ProcessGroupId, SessionId};
use roxy_terminal::TerminalOutput;
use spin::Once;

use file::TtyFile;
use tty::Tty;

/// One console terminal, bound to the kernel's selected output endpoint and keyboard manager.
///
/// The singleton is created between kernel terminal selection and process start; process 0/1/2
/// descriptors are opened from it by `kernel-main`.
struct Console {
    tty: Arc<Tty>,
}

static CONSOLE: Once<Arc<Console>> = Once::new();

/// Publishes the console terminal used for initial process descriptors.
///
/// Returns a `KeyboardListener` that main registers with the process-wide input manager.
///
/// # Panics
///
/// Panics when called more than once.
pub fn initialize(output: Arc<dyn TerminalOutput>) -> Arc<dyn KeyboardListener> {
    assert!(
        CONSOLE.get().is_none(),
        "console terminal initialized twice"
    );
    let tty = Arc::new(Tty::new(output));
    let console = Arc::new(Console { tty });
    CONSOLE.call_once(|| console.clone());
    console
}

impl KeyboardListener for Console {
    fn on_recive_input(&self, key: KeyEvent) {
        // IRQ context: interrupts are already disabled.
        self.tty.push_key(key);
        // Fast path: deliver VINTR/SIGINT immediately even when nobody is reading.
        self.tty.try_process_input_arrival();
        self.tty.observe_input();
    }
}

/// Opens an independent descriptor for the console terminal.
///
/// # Panics
///
/// Panics when the console has not been initialized.
#[must_use]
pub fn open() -> Arc<OpenFile> {
    let console = CONSOLE
        .get()
        .expect("console must be initialized before opening");

    TtyFile::open(console.tty.clone())
}

/// Binds the console terminal to a session as its controlling terminal, setting the session's
/// initial foreground process group.
///
/// This is how the composition root establishes the initial controlling terminal in the role
/// that a login/getty program plays on Linux: the terminal is acquired by a fresh session leader
/// (the spawned init shell) and its foreground group is the leader's group.
///
/// # Panics
///
/// Panics when the console has not been initialized or is already bound to a session.
pub fn bind_controlling_terminal(session: SessionId, pgid: ProcessGroupId) {
    let console = CONSOLE
        .get()
        .expect("console must be initialized before binding");

    console.tty.bind_session(session, pgid);
}

// Expose pass-throughs used by `TtyFile` through the concrete type.

#[cfg(feature = "kernel-test")]
mod test_support {
    use alloc::sync::Arc;
    use alloc::vec::Vec;

    use roxy_fd::{IoctlRequest, OpenFile};
    use roxy_keyboard_input::{KeyCode, KeyEvent, KeyState};
    use roxy_terminal::{OutputError, TerminalOutput};
    use roxy_tty_types::{ApplyWhen, LocalFlags, Termios, WindowSize};
    use spin::Mutex;

    use crate::Tty;
    use crate::file::TtyFile;

    pub(super) struct MockOutput {
        bytes: Mutex<Vec<u8>>,
        window_size: WindowSize,
    }

    impl MockOutput {
        pub(super) fn new() -> Self {
            Self {
                bytes: Mutex::new(Vec::new()),
                window_size: WindowSize {
                    rows: 30,
                    columns: 100,
                    pixel_width: 800,
                    pixel_height: 480,
                },
            }
        }

        pub(super) fn bytes(&self) -> Vec<u8> {
            self.bytes.lock().clone()
        }
    }

    impl TerminalOutput for MockOutput {
        fn write(&self, input: &[u8]) -> Result<usize, OutputError> {
            self.bytes.lock().extend_from_slice(input);
            Ok(input.len())
        }

        fn window_size(&self) -> WindowSize {
            self.window_size
        }
    }

    /// Build a raw key event for test injection.
    pub(super) fn key(code: KeyCode, state: KeyState) -> KeyEvent {
        KeyEvent { code, state }
    }

    /// Sets the line-discipline `echo`/`canonical` flags through the ioctl path.
    pub(super) fn set_settings(tty: &Arc<Tty>, echo: bool, canonical: bool) {
        let mut termios = Termios::default();
        tty.ioctl(IoctlRequest::GetTermios(&mut termios)).unwrap();
        termios.local_flags.set(LocalFlags::ECHO, echo);
        termios.local_flags.set(LocalFlags::ICANON, canonical);
        tty.ioctl(IoctlRequest::SetTermios {
            when: ApplyWhen::Immediate,
            termios,
        })
        .unwrap();
    }

    pub(super) fn open(events: Vec<KeyEvent>) -> (Arc<Tty>, Arc<MockOutput>, Arc<OpenFile>) {
        let output = Arc::new(MockOutput::new());
        let tty = Arc::new(Tty::new(output.clone()));
        // Inject events directly into the pending queue (no IRQ-path processing).
        for event in events {
            tty.push_key(event);
        }
        let file = TtyFile::open(tty.clone());

        (tty, output, file)
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_fd::IoctlRequest;
    use roxy_keyboard_input::{KeyCode, KeyEvent, KeyState};
    use roxy_test::kernel_test;
    use roxy_tty_types::WindowSize;

    use super::file::TtyFile;
    use super::test_support::{key, open, set_settings};

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
        set_settings(&tty, false, true);
        let mut buffer = [0; 2];

        assert_eq!(file.read(&mut buffer), Ok(2));
        assert_eq!(&buffer, b"x\n");
        assert!(output.bytes().is_empty());
    });

    kernel_test!("roxy-tty::initial-window-size", inherits_output_size, {
        let (tty, _output, file) = open(alloc::vec![]);
        let mut window_size = WindowSize::default();

        file.ioctl(IoctlRequest::GetWindowSize(&mut window_size))
            .unwrap();
        assert_eq!(
            window_size,
            WindowSize {
                rows: 30,
                columns: 100,
                pixel_width: 800,
                pixel_height: 480,
            }
        );
        let _ = &tty;
    });
}
