use alloc::sync::Arc;

use roxy_arch::{Architecture, CurrentArchitectureBackend};
use roxy_fd::{FileError, PollEvents};
use roxy_key_decoder::KeyDecoder;
use roxy_line_discipline::{LineDiscipline, ProcessResult};
use roxy_poll::{PollListener, PollRegistration};
use roxy_terminal::{OutputError, TerminalOutput};
use roxy_utils::Lock;

use crate::Tty;
use crate::encoder::{EncodedInputEvent, encode_decoded};

impl Tty {
    pub(super) fn new(output: Arc<dyn TerminalOutput>) -> Self {
        let window_size = output.window_size();

        Self {
            output,
            line_discipline: Lock::new(LineDiscipline::new()),
            decoder: Lock::new(KeyDecoder::new()),
            window_size: Lock::new(window_size),
            buffered: Lock::new(alloc::vec::Vec::new()),
            pending: Lock::new(heapless::Deque::new()),
            read_lock: Lock::new(()),
            poll_listeners: Arc::new(roxy_poll::PollListeners::new()),
            foreground_pgid: Lock::new(None),
            owner_session_id: Lock::new(None),
        }
    }

    pub(super) fn read(&self, output: &mut [u8]) -> Result<usize, FileError> {
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

    pub(super) fn poll(&self) -> Result<PollEvents, FileError> {
        let _read_guard = self.read_lock.lock();

        if self.buffered.lock().is_empty() {
            self.fill_buffered()?;
        }

        Ok(PollEvents {
            readable: !self.buffered.lock().is_empty(),
            writable: true,
            ..PollEvents::default()
        })
    }

    pub(super) fn register_poll_listener(&self, listener: Arc<PollListener>) -> PollRegistration {
        self.poll_listeners.register(listener)
    }

    pub(super) fn write(&self, input: &[u8]) -> Result<usize, FileError> {
        self.output.write(input).map_err(map_output_error)
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

    fn fill_buffered(&self) -> Result<(), FileError> {
        while self.buffered.lock().is_empty() {
            let Some(event) = self.next_input_event() else {
                return Ok(());
            };

            let result = self.line_discipline.lock().process(event.as_bytes());
            self.apply_result(event.as_bytes(), result)?;
        }

        Ok(())
    }

    /// Processes one pending input event in response to an input-arrival notification without
    /// blocking. Called from the IRQ handler's listener callback, so all locks use `try_lock`
    /// to avoid deadlock if the interrupted thread held them.
    ///
    /// This is the path that makes VINTR (Ctrl+C) deliver SIGINT immediately, even when nobody
    /// is reading the TTY (e.g. a foreground child is running).
    ///
    /// The decoder lock is acquired before the line-discipline lock, matching the read path's
    /// acquisition order (decoder in `next_input_event`, then discipline in `fill_buffered`).
    /// Events are only consumed after both locks are held, so a failed `try_lock` leaves the
    /// event in the pending queue for the read path to handle.
    pub(super) fn try_process_input_arrival(&self) {
        let Some(mut decoder) = self.decoder.try_lock() else {
            return;
        };
        let Some(mut discipline) = self.line_discipline.try_lock() else {
            return;
        };
        let Some(event) = self.pending.lock().pop_front() else {
            return;
        };
        let Some(decoded_key) = decoder.decode(event) else {
            return;
        };
        let Some(encoded) = encode_decoded(decoded_key) else {
            return;
        };
        let result = discipline.process(encoded.as_bytes());
        // Drop the decoder and discipline locks as soon as possible; echo and buffering use
        // separate locks.
        drop(decoder);
        drop(discipline);

        // Deliver the signal immediately — this is the whole point of interrupt-time processing.
        if let Some(signal) = result.signal {
            let pgid = *self.foreground_pgid.lock();
            match pgid {
                Some(pgid) => {
                    roxy_process::send_signal_to_pgid(pgid, signal);
                }
                None => {
                    // No foreground group selected; fall back to the reader. IRQ context may
                    // have no current thread (e.g. the only thread is blocked in waitpid), so
                    // skip when none can be resolved.
                    if let Some(reader) = roxy_process::try_current_process_id() {
                        let _ = roxy_process::send_signal(reader, signal);
                    }
                }
            }
        }

        // Buffer and echo are best-effort — if the lock is contended the read path will handle
        // the remaining events.
        let Some(mut buffered) = self.buffered.try_lock() else {
            return;
        };
        if let Some(buffer) = result.buffer {
            buffered.extend(buffer);
        }
        if result.echo {
            let _ = self.output.write(encoded.as_bytes());
        }
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

    /// Waits for the next input event and encodes it for the line discipline.
    ///
    /// Each raw key event is fed through the pc-keyboard layout decoder, which updates modifier
    /// state and produces either a Unicode character or a raw key. Releases of non-modifier keys
    /// yield `None` from the decoder and are skipped; modifier releases update state only.
    fn next_input_event(&self) -> Option<EncodedInputEvent> {
        loop {
            let event =
                CurrentArchitectureBackend::without_interrupts(|| self.pending.lock().pop_front())?;
            let decoded = self.decoder.lock().decode(event)?;
            if let Some(encoded) = encode_decoded(decoded) {
                return Some(encoded);
            }
        }
    }
}

fn map_output_error(error: OutputError) -> FileError {
    match error {
        OutputError::Io => FileError::Io,
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_fd::FileError;
    use roxy_input::{KeyCode, KeyState};
    use roxy_test::kernel_test;

    use crate::test_support::{key, open};

    kernel_test!("roxy-tty::noncanonical", preserves_event_stream, {
        let events = alloc::vec![
            key(KeyCode::A, KeyState::Pressed),
            key(KeyCode::ArrowLeft, KeyState::Pressed),
            key(KeyCode::ArrowLeft, KeyState::Released),
        ];
        let (tty, output, file) = open(events);
        tty.line_discipline.lock().settings.canonical = false;
        let mut first = [0; 1];
        let mut second = [0; 3];

        assert_eq!(file.read(&mut first), Ok(1));
        assert_eq!(&first, b"a");
        assert_eq!(file.read(&mut second), Ok(3));
        assert_eq!(&second, b"\x1b[D");
        assert_eq!(output.bytes(), b"a\x1b[D");
    });

    kernel_test!("roxy-tty::partial-echo", preserves_buffered_input, {
        let events = alloc::vec![key(KeyCode::ArrowLeft, KeyState::Pressed)];
        let (tty, output, file) = open(events);
        tty.line_discipline.lock().settings.canonical = false;
        output.limit_writes(1);
        let mut buffer = [0; 3];

        assert_eq!(file.read(&mut buffer), Err(FileError::Io));
        assert_eq!(output.bytes(), b"\x1b");
        assert_eq!(file.read(&mut buffer), Ok(3));
        assert_eq!(&buffer, b"\x1b[D");
    });

    kernel_test!("roxy-tty::echo-error", preserves_committed_line, {
        let (_tty, output, file) = open(alloc::vec![
            key(KeyCode::X, KeyState::Pressed),
            key(KeyCode::Return, KeyState::Pressed),
        ]);
        output.fail_on_call(2);
        let mut buffer = [0; 2];

        assert_eq!(file.read(&mut buffer), Err(FileError::Io));
        assert_eq!(output.bytes(), b"x");
        assert_eq!(file.read(&mut buffer), Ok(2));
        assert_eq!(&buffer, b"x\n");
    });
}
