use alloc::{sync::Arc, vec::Vec};

use roxy_arch::{Architecture, CurrentArchitectureBackend};
use roxy_fd::{FileError, PollEvents};
use roxy_line_discipline::{LineDiscipline, ProcessResult};
use roxy_poll::{PollListener, PollListeners, PollRegistration};
use roxy_process::{ProcessGroupId, SessionId};
use roxy_tty_types::WindowSize;
use roxy_utils::Lock;

use crate::input::TerminalInputSource;
use crate::output::{OutputError, TtyOutput};

/// The shared, byte-oriented core of a terminal.
///
/// One instance drives the line discipline, input buffering, blocking reads, output, ioctls, and
/// foreground-process-group/session semantics for **any** terminal — a console terminal or a pty
/// slave alike. (The core itself is not pty-specific; it is the terminal engine both kinds share.)
/// The only per-terminal differences — where bytes come from and where output/echo go — are
/// injected through [`TerminalInputSource`] and [`TtyOutput`].
pub struct TtyCore {
    /// Where processed output and echoed input are delivered.
    pub(crate) output: Arc<dyn TtyOutput>,
    /// Where line-discipline input bytes come from.
    pub(crate) input_source: Arc<dyn TerminalInputSource>,
    pub(crate) line_discipline: Lock<LineDiscipline>,
    pub(crate) window_size: Lock<WindowSize>,
    /// Bytes committed by the line discipline, waiting for a user read.
    pub(crate) buffered: Lock<Vec<u8>>,
    pub(crate) read_lock: Lock<()>,
    pub(crate) poll_listeners: Arc<PollListeners>,
    /// The process group that receives terminal-generated signals, when one is selected.
    pub(crate) foreground_pgid: Lock<Option<ProcessGroupId>>,
    /// The session that owns this controlling terminal, when one is established.
    pub(crate) owner_session_id: Lock<Option<SessionId>>,
}

impl TtyCore {
    /// Builds a terminal core around the given output endpoint and input source and registers it
    /// with the process's session-leader-exit handling so a controlling-session leader's exit
    /// releases the terminal and hangs up its foreground group.
    #[must_use]
    pub fn new(
        output: Arc<dyn TtyOutput>,
        input_source: Arc<dyn TerminalInputSource>,
    ) -> Arc<Self> {
        ensure_exit_handler();

        let core = Arc::new(Self {
            window_size: Lock::new(output.window_size()),
            output,
            input_source,
            line_discipline: Lock::new(LineDiscipline::new()),
            buffered: Lock::new(Vec::new()),
            read_lock: Lock::new(()),
            poll_listeners: Arc::new(PollListeners::new()),
            foreground_pgid: Lock::new(None),
            owner_session_id: Lock::new(None),
        });

        live_terminals().lock().push(Arc::downgrade(&core));

        core
    }

    /// Blocks reading committed input from this terminal.
    ///
    /// Returns on the first byte produced for `output`, `Interrupted` when a pending signal must be
    /// delivered at the userspace return boundary, or an I/O error from the foreground-read rule or
    /// echo writes.
    ///
    /// # Errors
    ///
    /// Returns `Interrupted` when a signal must be delivered, or `Io` from the foreground-read rule
    /// or an echo write.
    pub fn read(&self, output: &mut [u8]) -> Result<usize, FileError> {
        if output.is_empty() {
            return Ok(0);
        }

        loop {
            // Background process groups reading from the terminal receive SIGTTIN; if the
            // signal is blocked or ignored the read fails with EIO instead.
            self.ensure_foreground_read()?;

            let count = self.read_inner(output)?;

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

    /// Injects bytes of **user/input-stream** data, as if typed by the user, and runs them through
    /// the line discipline.
    ///
    /// This is the reverse of [`TtyCore::write`]: `process_input` feeds the terminal **input** (a pty
    /// master write, or the console's decoded keyboard events), while `write` carries the terminal
    /// program's **output** the other way. The input is buffered for reads, echoed, and any generated
    /// signal is delivered to the foreground process group. Runs from a normal context and takes
    /// full locks.
    ///
    /// # Errors
    ///
    /// Returns `Io` when echoing the input to the output endpoint fails or is partial.
    pub fn process_input(&self, input: &[u8]) -> Result<(), FileError> {
        if input.is_empty() {
            return Ok(());
        }
        self.apply_bytes(input)
    }

    /// IRQ/callback-safe fast path: try to consume and apply one input immediately.
    ///
    /// This is what makes VINTR (Ctrl+C) deliver SIGINT even when nobody is reading the terminal
    /// (e.g. a foreground child is running). It peeks the next input, then acquires the line
    /// discipline lock; on failure it leaves the input pending for the read path, so an input is
    /// never lost.
    pub fn try_process_input_arrival(&self) {
        let Some(input) = self.input_source.try_peek_bytes() else {
            return;
        };
        let Some(mut discipline) = self.line_discipline.try_lock() else {
            return;
        };
        self.input_source.consume_peeked();

        let result = discipline.process(&input);
        drop(discipline);

        if let Some(signal) = result.signal {
            match *self.foreground_pgid.lock() {
                Some(pgid) => {
                    roxy_process::send_signal_to_pgid(pgid, signal);
                }
                None => {
                    // No foreground group selected; fall back to the reader. IRQ context may
                    // have no current thread, so skip when none can be resolved.
                    if let Some(reader) = roxy_process::try_current_process_id() {
                        let _ = roxy_process::send_signal(reader, signal);
                    }
                }
            }
        }

        // Buffer and echo are best-effort — if the lock is contended the read path will handle
        // the remaining events.
        if let (Some(buffer), Some(mut buffered)) = (result.buffer, self.buffered.try_lock()) {
            buffered.extend(buffer);
        }
        if result.echo {
            let _ = self.output.write(&input);
        }
    }

    /// Wakes any reader blocked in [`Self::read`] or a poll registration.
    ///
    /// The input source (or a pty master writing) must call this when it has injected input.
    pub fn observe_input(&self) {
        self.poll_listeners.notify();
    }

    /// Reports the current readiness of this terminal.
    ///
    /// # Errors
    ///
    /// Returns `Io` when pulling and processing pending input fails on an echo write.
    pub fn poll(&self) -> Result<PollEvents, FileError> {
        let _read_guard = self.read_lock.lock();

        if self.buffered.lock().is_empty() {
            self.fill_buffered()?;
        }

        Ok(PollEvents {
            readable: !self.buffered.lock().is_empty(),
            // Terminal output has no backpressure model, so it is always writable.
            writable: true,
            ..PollEvents::default()
        })
    }

    /// Registers a listener to be notified when readiness may have changed.
    pub fn register_poll_listener(&self, listener: Arc<PollListener>) -> PollRegistration {
        self.poll_listeners.register(listener)
    }

    /// Writes the terminal program's **output** bytes to the output endpoint (`TerminalOutput` for a
    /// console, the pty master's receive buffer for a pty slave).
    ///
    /// This is the reverse of [`TtyCore::process_input`] (which feeds input in); `write` carries
    /// program output out. It is also how a descriptor's `File::write` reaches the terminal.
    ///
    /// # Errors
    ///
    /// Returns the output endpoint's `Io` error.
    pub fn write(&self, output: &[u8]) -> Result<usize, FileError> {
        self.output.write(output).map_err(map_output_error)
    }

    /// Returns the session currently owning this terminal, if any.
    #[must_use]
    pub fn owning_session(&self) -> Option<SessionId> {
        *self.owner_session_id.lock()
    }

    /// Binds the terminal to a session as its controlling terminal, setting the initial foreground
    /// process group.
    ///
    /// # Panics
    ///
    /// Panics when the terminal is already bound to a session.
    pub fn bind_session(&self, session: SessionId, pgid: ProcessGroupId) {
        let mut current_session = self.owner_session_id.lock();

        assert!(
            current_session.is_none(),
            "terminal controlling session bound twice"
        );
        *current_session = Some(session);
        *self.foreground_pgid.lock() = Some(pgid);
    }

    /// Releases the terminal when its controlling session's leader exits.
    ///
    /// If `session` owns this terminal, clears the ownership and returns the process group that
    /// should receive `SIGHUP`. Returns `None` when the terminal is owned by another session.
    pub fn release_on_session_leader_exit(&self, session: SessionId) -> Option<ProcessGroupId> {
        let owns_terminal = *self.owner_session_id.lock() == Some(session);
        if !owns_terminal {
            return None;
        }

        *self.owner_session_id.lock() = None;

        self.foreground_pgid.lock().take()
    }

    fn read_inner(&self, output: &mut [u8]) -> Result<usize, FileError> {
        let _read_guard = self.read_lock.lock();
        let count = self.drain_buffered(output);
        if count > 0 {
            return Ok(count);
        }

        self.fill_buffered()?;
        Ok(self.drain_buffered(output))
    }

    /// Enforces the foreground read rule: a background process group reading the controlling
    /// terminal is sent SIGTTIN (which stops it via the default action). If SIGTTIN is blocked
    /// or ignored the read fails with EIO instead, matching Linux `n_tty`/`tty_check_change`.
    ///
    /// Returns `Ok(())` for the foreground group (or when no foreground group has been
    /// established yet, i.e. the terminal has no controlling session). Returns `Interrupted`
    /// after queuing SIGTTIN so the signal is delivered at the return boundary.
    fn ensure_foreground_read(&self) -> Result<(), FileError> {
        let Some(foreground) = *self.foreground_pgid.lock() else {
            return Ok(());
        };
        let caller_pgid = roxy_process::current_process_group_id();

        if foreground == caller_pgid {
            return Ok(());
        }

        // Background read: block or ignore SIGTTIN to avoid stopping ourselves.
        let sigttin = roxy_signal::Signal::TerminalInput;
        let blocked = roxy_process::currently_blocked_signals()
            .contains(roxy_signal::SignalSet::from_signal(sigttin));
        let ignored = matches!(
            roxy_process::signal_action_of(sigttin),
            roxy_process::SignalAction::Ignore
        );

        if blocked || ignored {
            return Err(FileError::Io);
        }

        roxy_process::send_signal_to_pgid(caller_pgid, sigttin);
        Err(FileError::Interrupted)
    }

    /// Pulls input from the source and processes it until committed input is available.
    fn fill_buffered(&self) -> Result<(), FileError> {
        while self.buffered.lock().is_empty() {
            let Some(input) = self.input_source.next_input_bytes() else {
                return Ok(());
            };

            self.apply_bytes(&input)?;
        }

        Ok(())
    }

    /// Runs one input payload through the line discipline and applies the result.
    fn apply_bytes(&self, input: &[u8]) -> Result<(), FileError> {
        if input.is_empty() {
            return Ok(());
        }

        let result = self.line_discipline.lock().process(input);

        self.apply_result(input, result)
    }

    fn apply_result(&self, input: &[u8], result: ProcessResult) -> Result<(), FileError> {
        if let Some(signal) = result.signal {
            match *self.foreground_pgid.lock() {
                Some(pgid) => {
                    roxy_process::send_signal_to_pgid(pgid, signal);
                }
                None => {
                    // TODO(foreground-process-group): no group selected; fall back to the reader.
                    let _ = roxy_process::send_signal(roxy_process::current_process_id(), signal);
                }
            }
        }

        if let Some(buffer) = result.buffer {
            self.buffered.lock().extend(buffer);
        }

        if result.echo {
            let written = self.output.write(input).map_err(map_output_error)?;

            if written != input.len() {
                return Err(FileError::Io);
            }
        }

        Ok(())
    }

    fn drain_buffered(&self, output: &mut [u8]) -> usize {
        let mut buffered = self.buffered.lock();
        let count = output.len().min(buffered.len());

        output[..count].copy_from_slice(&buffered[..count]);

        let remaining = buffered.len() - count;

        buffered.copy_within(count.., 0);
        buffered.truncate(remaining);

        count
    }
}

fn map_output_error(error: OutputError) -> FileError {
    match error {
        OutputError::Io => FileError::Io,
    }
}

/// Weak references to every live terminal core, shared across console and pty terminals.
///
/// The process layer calls a single session-leader-exit handler; that handler scans these cores
/// and releases each one owned by the exited session, sending `SIGHUP` to its foreground group.
/// A weak set (rather than a per-session index) keeps the handler's bookkeeping simple while
/// there are few live terminals.
static LIVE_TERMINALS: spin::Once<Lock<alloc::vec::Vec<alloc::sync::Weak<TtyCore>>>> =
    spin::Once::new();

fn live_terminals() -> &'static Lock<alloc::vec::Vec<alloc::sync::Weak<TtyCore>>> {
    LIVE_TERMINALS.call_once(|| Lock::new(alloc::vec::Vec::new()))
}

static EXIT_HANDLER_INSTALLED: spin::Once<()> = spin::Once::new();

fn ensure_exit_handler() {
    EXIT_HANDLER_INSTALLED.call_once(|| {
        roxy_process::register_session_leader_exit_handler(on_session_leader_exit);
    });
}

/// Sends `SIGHUP` to the foreground process group of every terminal whose controlling session's
/// leader just exited, then releases those terminals (Linux `disassociate_ctty`/hangup
/// semantics).
///
/// Runs on the exiting session leader's own thread via the process-side callback, so no
/// process-table lock is held; the locks taken here are the terminals' own.
fn on_session_leader_exit(session: SessionId) {
    // Snapshot the live set, pruning references whose terminal has already been dropped.
    let terminals = {
        let mut live = live_terminals().lock();
        let clone = live.clone();
        live.retain(|terminal| terminal.strong_count() > 0);
        clone
    };

    for weak in terminals {
        let Some(core) = weak.upgrade() else {
            continue;
        };
        if let Some(foreground) = core.release_on_session_leader_exit(session) {
            roxy_process::send_signal_to_pgid(foreground, roxy_signal::Signal::Hangup);
        }
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use crate::test_support::open;

    roxy_test::kernel_test!("roxy-tty-core::canonical-commit", commits_on_newline, {
        let (core, source, output) = open();
        source.push(b"ab\n");
        let mut buffer = [0; 8];

        assert_eq!(core.read(&mut buffer), Ok(3));
        assert_eq!(&buffer[..3], b"ab\n");
        assert_eq!(output.bytes(), b"ab\n");
    });

    roxy_test::kernel_test!("roxy-tty-core::echo-disabled", skips_echo, {
        let (core, source, output) = open();
        core.line_discipline.lock().settings.echo = false;
        source.push(b"x\n");
        let mut buffer = [0; 2];

        assert_eq!(core.read(&mut buffer), Ok(2));
        assert_eq!(&buffer, b"x\n");
        assert!(output.bytes().is_empty());
    });

    roxy_test::kernel_test!("roxy-tty-core::window-size", inherits_output_size, {
        let (core, _source, _output) = open();

        assert_eq!(
            *core.window_size.lock(),
            roxy_tty_types::WindowSize {
                rows: 30,
                columns: 100,
                pixel_width: 800,
                pixel_height: 480,
            }
        );
    });
}
