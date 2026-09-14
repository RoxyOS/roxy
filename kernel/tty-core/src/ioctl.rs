use roxy_fd::{IoctlError, IoctlRequest};
use roxy_line_discipline::LineDisciplineSettings;
use roxy_process::ProcessGroupId;
use roxy_tty_types::{ApplyWhen, LocalFlags, Termios};

use crate::core::TtyCore;

// termios flag bit values follow mlibc `sysdeps/roxy/include/abi-bits/termios.h`. Only bits with
// real, implemented line-discipline semantics (or, below, bits that are genuinely inapplicable on a
// pty and documented as `TODO`) are accepted; everything else is rejected via the centralized
// unsupported diagnostic.
const CS8: u32 = 0o60;
/// `c_iflag` ICRNL: map input CR to NL.
const ICRNL: u32 = 0o400;
/// `c_iflag` INLCR: map input NL to CR.
const INLCR: u32 = 0o100;
/// `c_iflag` IGNCR: discard input CR.
const IGNCR: u32 = 0o200;
/// `c_oflag` OPOST: enable output post-processing.
const OPOST: u32 = 0o1;
/// `c_oflag` ONLCR: map output NL to CR+NL (effective under OPOST).
const ONLCR: u32 = 0o4;
const VINTR: usize = 0;
const VERASE: usize = 2;
const VMIN: usize = 6;

/// `c_cflag` bits that control the modem/line this terminal runs on. A pty has no modem, parity, or
/// line speed, so these are accepted and treated as no-ops rather than rejected. CRTSCTS hardware
/// flow control and CMSPAR (0o4000000000) are omitted because `tcflag_t` is u32 on this platform
/// and those values overflow; they are not used by the pty anyway.
const MODEM_CFLAG: u32 = CS8
    | 0o100 /* CSTOPB */
    | 0o200 /* CREAD */
    | 0o400 /* PARENB */
    | 0o1000 /* PARODD */
    | 0o2000 /* HUPCL */
    | 0o4000 /* CLOCAL */
    | 0o10017 /* CBAUD */
    | 0o10000 /* CBAUDEX */;

/// `c_lflag` bits whose effect is cosmetic or currently unimplemented (echo charm, flow control,
/// output-to-background gating). They are accepted so a cooked terminal (e.g. xterm) can configure
/// itself, with `TODO` markers for the semantics not yet implemented.
const ECHO_LFLAG: u32 = 0o10 /* ECHO */
    | 0o20 /* ECHOE */
    | 0o40 /* ECHOK */
    | 0o100 /* ECHONL */
    | 0o1000 /* ECHOCTL */
    | 0o2000 /* ECHOPRT */
    | 0o4000 /* ECHOKE */
    | 0o10_000 /* FLUSHO */
    | 0o40_000 /* PENDIN */
    | 0o200_000 /* EXTPROC */
    | 0o400 /* TOSTOP */
    | 0o200 /* NOFLSH */
    | 0o100_000; /* IEXTEN */

impl TtyCore {
    /// Dispatches a terminal ioctl request.
    ///
    /// # Errors
    ///
    /// Returns `NotTty` for unsupported requests or when the terminal has no controlling session,
    /// or `Invalid`/`Unsupported` for request arguments the terminal rejects.
    pub fn ioctl(&self, request: IoctlRequest<'_>) -> Result<(), IoctlError> {
        match request {
            IoctlRequest::GetTermios(termios) => {
                *termios = self.termios();
                Ok(())
            }
            IoctlRequest::SetTermios { when, termios } => {
                self.set_termios(when, termios)?;
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
            IoctlRequest::FbGetInfo(_)
            | IoctlRequest::FbTakeControl
            | IoctlRequest::FbReleaseControl => Err(IoctlError::NotTty),
        }
    }

    fn termios(&self) -> Termios {
        let settings = self.line_discipline.lock().settings;

        let mut termios = termios_from_settings(settings);
        // Return the full stored control-character set so tcgetattr reflects what tcsetattr saved.
        termios.control_characters = *self.control_characters.lock();
        termios
    }

    fn set_termios(&self, when: ApplyWhen, termios: Termios) -> Result<(), IoctlError> {
        validate_termios(&termios)?;
        // Persist the control characters for round-trip; only VINTR/VERASE drive the discipline.
        *self.control_characters.lock() = termios.control_characters;

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

            discipline.update_settings(settings_from_termios(termios))
        };

        if let Some(released) = released {
            self.buffered.lock().extend(released);
        }

        Ok(())
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

fn termios_from_settings(settings: LineDisciplineSettings) -> Termios {
    let mut control_characters = [0; 32];
    control_characters[VINTR] = settings.intr_character;
    control_characters[VERASE] = settings.erase_character;
    control_characters[VMIN] = 1;

    Termios {
        input_flags: (if settings.icrnl { ICRNL } else { 0 })
            | (if settings.inlcr { INLCR } else { 0 })
            | (if settings.igncr { IGNCR } else { 0 }),
        output_flags: (if settings.opost { OPOST } else { 0 })
            | (if settings.onlcr { ONLCR } else { 0 }),
        control_flags: CS8,
        local_flags: local_flags_from_settings(settings),
        line_discipline: 0,
        control_characters,
        input_speed: 0,
        output_speed: 0,
    }
}

fn settings_from_termios(termios: Termios) -> LineDisciplineSettings {
    LineDisciplineSettings {
        echo: termios.local_flags.contains(LocalFlags::ECHO),
        canonical: termios.local_flags.contains(LocalFlags::ICANON),
        erase_character: termios.control_characters[VERASE],
        isig: termios.local_flags.contains(LocalFlags::ISIG),
        intr_character: termios.control_characters[VINTR],
        icrnl: termios.input_flags & ICRNL != 0,
        inlcr: termios.input_flags & INLCR != 0,
        igncr: termios.input_flags & IGNCR != 0,
        opost: termios.output_flags & OPOST != 0,
        onlcr: termios.output_flags & ONLCR != 0,
    }
}

fn local_flags_from_settings(settings: LineDisciplineSettings) -> LocalFlags {
    let mut flags = LocalFlags::empty();
    flags.set(LocalFlags::ECHO, settings.echo);
    flags.set(LocalFlags::ICANON, settings.canonical);
    flags.set(LocalFlags::ISIG, settings.isig);

    flags
}

/// Validate that all fields in `termios` are supported. Returns `Unsupported` if not.
#[allow(clippy::similar_names)] // supported_iflag/oflag/cflag/lflag are distinct flag groups
fn validate_termios(termios: &Termios) -> Result<(), IoctlError> {
    // c_iflag: implemented CR/NL translation plus pty-inapplicable flow/break bits (TODO
    // flow-control/parity semantics are not yet implemented; they are accepted to let a cooked
    // terminal configure itself).
    let supported_iflag = ICRNL | INLCR | IGNCR
        | 0o1 /* IGNBRK */ | 0o2 /* BRKINT */ | 0o4 /* IGNPAR */ | 0o10 /* PARMRK */
        | 0o20 /* INPCK */ | 0o40 /* ISTRIP */ | 0o1000 /* IUCLC */ | 0o2000 /* IXON */
        | 0o4000 /* IXANY */ | 0o10000 /* IXOFF */ | 0o20000 /* IMAXBEL */ | 0o40000 /* IUTF8 */;
    validate_fixed(
        "ioctl.tcsetattr.input-flags",
        termios.input_flags & !supported_iflag,
        0,
    )?;
    // c_oflag: OPOST/ONLCR implemented; the remaining output post-processing bits (TODO) are
    // accepted as no-ops so cooked terminals can configure their CR/NL/fluid line endings.
    let supported_oflag = OPOST | ONLCR
        | 0o2 /* OLCUC */ | 0o10 /* OCRNL */ | 0o20 /* ONOCR */ | 0o40 /* ONLRET */
        | 0o100 /* OFILL */ | 0o200 /* OFDEL */;
    validate_fixed(
        "ioctl.tcsetattr.output-flags",
        termios.output_flags & !supported_oflag,
        0,
    )?;
    // c_cflag: a pty has no modem/parity/line speed, so those bits are accepted as no-ops.
    validate_fixed(
        "ioctl.tcsetattr.control-flags",
        termios.control_flags & !MODEM_CFLAG,
        0,
    )?;
    // c_lflag: ISIG/ICANON/ECHO implemented; the rest (echo charm, flow control, background-output
    // gating) are accepted as no-ops with TODO markers.
    let supported_lflag =
        (LocalFlags::ISIG | LocalFlags::ICANON | LocalFlags::ECHO).bits() | ECHO_LFLAG;
    validate_fixed(
        "ioctl.tcsetattr.local-flags",
        termios.local_flags.bits() & !supported_lflag,
        0,
    )?;
    validate_fixed(
        "ioctl.tcsetattr.line-discipline",
        u32::from(termios.line_discipline),
        0,
    )?;
    validate_fixed("ioctl.tcsetattr.input-speed", termios.input_speed, 0)?;
    validate_fixed("ioctl.tcsetattr.output-speed", termios.output_speed, 0)?;

    // The control characters are single bytes (cc_t) and are round-tripped wholesale; only VINTR
    // (SIGINT) and VERASE drive the discipline.
    // TODO(control-chars): SIGQUIT/SIGTSTP (VQUIT/VSUSP), VEOF, VKILL, VSTART/VSTOP, etc. are
    // stored but not yet wired to the line discipline.
    Ok(())
}

/// The default termios control-character set: VINTR is Ctrl+C (3), VERASE backspace (8), VMIN 1.
pub(crate) fn default_control_characters() -> [u8; 32] {
    let mut cc = [0; 32];
    cc[VINTR] = 0o3;
    cc[VERASE] = 0o10;
    cc[VMIN] = 1;
    cc
}

fn validate_fixed(operation: &'static str, actual: u32, expected: u32) -> Result<(), IoctlError> {
    if actual != expected {
        return Err(IoctlError::Unsupported {
            operation,
            argument: u64::from(actual),
        });
    }

    Ok(())
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_fd::{IoctlError, IoctlRequest};
    use roxy_tty_types::{ApplyWhen, LocalFlags, Termios, WindowSize};

    use super::{ONLCR, OPOST};
    use crate::test_support::open;

    /// `c_oflag` NLDLY (newline delay), an output flag outside the accepted mask: `OPOST`/`ONLCR`
    /// are implemented and the remaining post-processing bits are accepted as no-ops.
    const NLDLY: u32 = 0o400;

    roxy_test::kernel_test!("roxy-tty-core::termios-ioctl", updates_input_mode, {
        let (core, source, output) = open();
        source.push(b"x");
        let mut termios = Termios::default();

        core.ioctl(IoctlRequest::GetTermios(&mut termios)).unwrap();

        termios.local_flags = LocalFlags::empty();
        assert_eq!(
            core.ioctl(IoctlRequest::SetTermios {
                when: ApplyWhen::Immediate,
                termios,
            }),
            Ok(())
        );

        let mut input = [0; 1];
        assert_eq!(core.read(&mut input), Ok(1));
        assert_eq!(&input, b"x");
        assert!(output.bytes().is_empty());
    });

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
        "roxy-tty-core::termios-output-flags",
        round_trips_output_flags,
        {
            let (core, _source, _output) = open();
            let mut termios = Termios::default();

            core.ioctl(IoctlRequest::GetTermios(&mut termios)).unwrap();
            // The default terminal advertises the output post-processing it implements.
            assert_eq!(termios.output_flags, OPOST | ONLCR);

            termios.output_flags = OPOST;
            assert_eq!(
                core.ioctl(IoctlRequest::SetTermios {
                    when: ApplyWhen::Immediate,
                    termios,
                }),
                Ok(())
            );

            core.ioctl(IoctlRequest::GetTermios(&mut termios)).unwrap();
            assert_eq!(termios.output_flags, OPOST);
        }
    );

    roxy_test::kernel_test!(
        "roxy-tty-core::termios-output-unsupported",
        rejects_output_flags_outside_the_mask,
        {
            let (core, _source, _output) = open();
            let mut termios = Termios::default();

            core.ioctl(IoctlRequest::GetTermios(&mut termios)).unwrap();
            termios.output_flags = NLDLY;

            assert_eq!(
                core.ioctl(IoctlRequest::SetTermios {
                    when: ApplyWhen::Immediate,
                    termios,
                }),
                Err(IoctlError::Unsupported {
                    operation: "ioctl.tcsetattr.output-flags",
                    argument: u64::from(NLDLY),
                })
            );
        }
    );

    roxy_test::kernel_test!("roxy-tty-core::termios-unsupported", rejects_input_flags, {
        let (core, _source, _output) = open();
        let mut termios = Termios::default();

        core.ioctl(IoctlRequest::GetTermios(&mut termios)).unwrap();
        termios.input_flags = 1;

        assert_eq!(
            core.ioctl(IoctlRequest::SetTermios {
                when: ApplyWhen::Immediate,
                termios,
            }),
            Err(IoctlError::Unsupported {
                operation: "ioctl.tcsetattr.input-flags",
                argument: 1,
            })
        );
    });
}
