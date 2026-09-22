use roxy_fd::{IoctlError, IoctlRequest};
use roxy_line_discipline::LineDisciplineSettings;
use roxy_process::ProcessGroupId;
use roxy_tty_types::{ApplyWhen, TerminalAttributes, TerminalFlags};

use crate::core::TtyCore;

impl TtyCore {
    /// Dispatches a terminal ioctl request.
    ///
    /// # Errors
    ///
    /// Returns `NotTty` for unsupported requests or when the terminal has no controlling session,
    /// or `Invalid`/`Unsupported` for request arguments the terminal rejects.
    pub fn ioctl(&self, request: IoctlRequest<'_>) -> Result<(), IoctlError> {
        match request {
            IoctlRequest::GetTerminalAttributes(attributes) => {
                *attributes = self.terminal_attributes();
                Ok(())
            }
            IoctlRequest::SetTerminalAttributes { when, attributes } => {
                self.set_terminal_attributes(when, attributes);
                Ok(())
            }
            IoctlRequest::GetWindowSize(window_size) => {
                *window_size = *self.window_size.lock();
                Ok(())
            }
            IoctlRequest::SetWindowSize(window_size) => {
                *self.window_size.lock() = window_size;
                Ok(())
            }
            IoctlRequest::GetForegroundPgid(pgid) => {
                *pgid = self
                    .foreground_pgid
                    .lock()
                    .ok_or(IoctlError::NotTty)?
                    .as_u64()
                    .try_into()
                    .map_err(|_| IoctlError::Invalid)?;
                Ok(())
            }
            IoctlRequest::Tcflush(which) => self.tcflush(which),
            IoctlRequest::SetForegroundPgid(pgid) => {
                // The caller must have this terminal as its controlling terminal and belong to
                // the terminal's session (Linux `tiocspgrp`). The target group must exist and
                // belong to the same session.
                let session = (*self.owner_session_id.lock()).ok_or(IoctlError::NotTty)?;
                let caller_session =
                    roxy_process::current_process_session_id().ok_or(IoctlError::NotTty)?;
                if caller_session != session {
                    return Err(IoctlError::NotTty);
                }

                let pgid = ProcessGroupId::new(u64::from(pgid)).ok_or(IoctlError::Invalid)?;
                let target_session =
                    roxy_process::process_session_of_pgid(pgid).ok_or(IoctlError::Invalid)?;
                if target_session != session {
                    return Err(IoctlError::Invalid);
                }

                // POSIX: a background process group calling TIOCSPGRP must be sent SIGTTOU
                // (unless the signal is blocked or ignored).
                let caller_pgid = roxy_process::current_process_group_id();
                if *self.foreground_pgid.lock() != Some(caller_pgid) {
                    let sigttou = roxy_signal::Signal::TerminalOutput;
                    let blocked = roxy_process::currently_blocked_signals()
                        .contains(roxy_signal::SignalSet::from_signal(sigttou));
                    let ignored = matches!(
                        roxy_process::signal_action_of(sigttou),
                        roxy_process::SignalAction::Ignore
                    );
                    if !blocked && !ignored {
                        roxy_process::send_signal_to_pgid(caller_pgid, sigttou);
                    }
                }

                *self.foreground_pgid.lock() = Some(pgid);
                Ok(())
            }
            IoctlRequest::SetControllingTerminal { force } => self.set_controlling_terminal(force),
            IoctlRequest::GetTerminalName(output) => {
                let Some(path) = self.terminal_path else {
                    return Err(IoctlError::NotTty);
                };
                output.extend_from_slice(path);
                output.push(0);
                Ok(())
            }
            IoctlRequest::FbGetInfo(_)
            | IoctlRequest::FbTakeControl
            | IoctlRequest::FbReleaseControl => Err(IoctlError::NotTty),
        }
    }

    fn terminal_attributes(&self) -> TerminalAttributes {
        attributes_from_settings(self.line_discipline.lock().settings)
    }

    fn set_terminal_attributes(&self, when: ApplyWhen, attributes: TerminalAttributes) {
        let _read_guard = self.read_lock.lock();

        if when == ApplyWhen::Flush {
            self.buffered.lock().clear();
            // Discard the source's pending input. The source's discard is IRQ-safe.
            self.input_source.discard_pending_input();
        }

        // Buffered input inside line discipline
        let released = {
            let mut discipline = self.line_discipline.lock();

            if when == ApplyWhen::Flush {
                discipline.clear_input();
            }

            discipline.update_settings(settings_from_attributes(attributes))
        };

        if let Some(released) = released {
            self.buffered.lock().extend(released);
        }
    }
    /// Flushes queued terminal input/output (`TCFLSH`): `which` is `TCIFLUSH` (0, discard
    /// unread input), `TCOFLUSH` (1, discard unwritten output — a no-op here, since output goes
    /// through the endpoint without buffering), or `TCIOFLUSH` (2, both).
    fn tcflush(&self, which: u32) -> Result<(), IoctlError> {
        let flush_input = match which {
            0 /* TCIFLUSH */ | 2 /* TCIOFLUSH */ => true,
            1 /* TCOFLUSH */ => false,
            _ => return Err(IoctlError::Invalid),
        };

        if flush_input {
            self.buffered.lock().clear();
            self.line_discipline.lock().clear_input();
            self.input_source.discard_pending_input();
        }

        Ok(())
    }

    /// Makes the calling process's session the controller of this terminal (`TIOCSCTTY`).
    ///
    /// On success the terminal's `owner_session_id` and its initial foreground process group
    /// are bound to the caller's session, making this terminal that session's controlling
    /// terminal. Only a session leader may do this, and the terminal must not already be
    /// owned unless `force` requests stealing it.
    fn set_controlling_terminal(&self, force: bool) -> Result<(), IoctlError> {
        // The caller must be a session leader that does not already have a controlling terminal.
        if !roxy_process::is_current_session_leader() {
            return Err(IoctlError::NotTty);
        }

        let mut session = self.owner_session_id.lock();
        if session.is_some() && !force {
            return Err(IoctlError::NotTty);
        }

        let caller_pgid = roxy_process::current_process_group_id();
        let caller_session =
            roxy_process::current_process_session_id().ok_or(IoctlError::NotTty)?;

        *session = Some(caller_session);
        *self.foreground_pgid.lock() = Some(caller_pgid);

        Ok(())
    }
}

fn attributes_from_settings(settings: LineDisciplineSettings) -> TerminalAttributes {
    let mut flags = TerminalFlags::empty();
    flags.set(TerminalFlags::ECHO, settings.echo);
    flags.set(TerminalFlags::ICANON, settings.canonical);
    flags.set(TerminalFlags::ISIG, settings.isig);
    flags.set(TerminalFlags::ICRNL, settings.icrnl);
    flags.set(TerminalFlags::INLCR, settings.inlcr);
    flags.set(TerminalFlags::IGNCR, settings.igncr);
    flags.set(TerminalFlags::OPOST, settings.opost);
    flags.set(TerminalFlags::ONLCR, settings.onlcr);

    TerminalAttributes {
        flags,
        interrupt_byte: settings.intr_character,
        erase_byte: settings.erase_character,
    }
}

fn settings_from_attributes(attributes: TerminalAttributes) -> LineDisciplineSettings {
    let flags = attributes.flags;

    LineDisciplineSettings {
        echo: flags.contains(TerminalFlags::ECHO),
        canonical: flags.contains(TerminalFlags::ICANON),
        erase_character: attributes.erase_byte,
        isig: flags.contains(TerminalFlags::ISIG),
        intr_character: attributes.interrupt_byte,
        icrnl: flags.contains(TerminalFlags::ICRNL),
        inlcr: flags.contains(TerminalFlags::INLCR),
        igncr: flags.contains(TerminalFlags::IGNCR),
        opost: flags.contains(TerminalFlags::OPOST),
        onlcr: flags.contains(TerminalFlags::ONLCR),
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_fd::IoctlRequest;
    use roxy_tty_types::{ApplyWhen, TerminalAttributes, TerminalFlags, WindowSize};

    use crate::test_support::open;

    roxy_test::kernel_test!(
        "roxy-tty-core::terminal-attributes-ioctl",
        updates_input_mode,
        {
            let (core, source, output) = open();
            source.push(b"x");
            let mut attributes = TerminalAttributes::default();

            core.ioctl(IoctlRequest::GetTerminalAttributes(&mut attributes))
                .unwrap();

            attributes.flags = TerminalFlags::empty();
            core.ioctl(IoctlRequest::SetTerminalAttributes {
                when: ApplyWhen::Immediate,
                attributes,
            })
            .unwrap();

            let mut input = [0; 1];
            assert_eq!(core.read(&mut input), Ok(1));
            assert_eq!(&input, b"x");
            assert!(output.bytes().is_empty());
        }
    );

    roxy_test::kernel_test!(
        "roxy-tty-core::terminal-attributes-default",
        reports_the_default_attributes,
        {
            let (core, _source, _output) = open();
            let mut attributes = TerminalAttributes::default();

            core.ioctl(IoctlRequest::GetTerminalAttributes(&mut attributes))
                .unwrap();

            assert_eq!(
                attributes.flags,
                TerminalFlags::ISIG
                    | TerminalFlags::ICANON
                    | TerminalFlags::ECHO
                    | TerminalFlags::OPOST
                    | TerminalFlags::ONLCR
                    | TerminalFlags::ICRNL
            );
            assert_eq!(attributes.interrupt_byte, 0o3);
            assert_eq!(attributes.erase_byte, 0o10);
        }
    );

    roxy_test::kernel_test!("roxy-tty-core::winsize-ioctl", round_trips_window_size, {
        let (core, _source, _output) = open();
        let size = WindowSize {
            rows: 40,
            columns: 120,
            pixel_width: 960,
            pixel_height: 640,
        };

        assert_eq!(core.ioctl(IoctlRequest::SetWindowSize(size)), Ok(()));

        let mut returned = WindowSize::default();
        assert_eq!(
            core.ioctl(IoctlRequest::GetWindowSize(&mut returned)),
            Ok(())
        );
        assert_eq!(returned, size);
    });

    roxy_test::kernel_test!(
        "roxy-tty-core::terminal-attributes-opost",
        round_trips_output_post_processing,
        {
            let (core, _source, _output) = open();
            let mut attributes = TerminalAttributes::default();

            core.ioctl(IoctlRequest::GetTerminalAttributes(&mut attributes))
                .unwrap();
            // The default terminal advertises the output post-processing it implements.
            assert!(attributes.flags.contains(TerminalFlags::OPOST));
            assert!(attributes.flags.contains(TerminalFlags::ONLCR));

            attributes.flags.remove(TerminalFlags::ONLCR);
            core.ioctl(IoctlRequest::SetTerminalAttributes {
                when: ApplyWhen::Immediate,
                attributes,
            })
            .unwrap();

            core.ioctl(IoctlRequest::GetTerminalAttributes(&mut attributes))
                .unwrap();
            assert!(attributes.flags.contains(TerminalFlags::OPOST));
            assert!(!attributes.flags.contains(TerminalFlags::ONLCR));
        }
    );
}
