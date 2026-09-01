use roxy_arch::{Architecture, CurrentArchitectureBackend};
use roxy_fd::{IoctlError, IoctlRequest};
use roxy_line_discipline::LineDisciplineSettings;
use roxy_process::ProcessGroupId;
use roxy_tty_types::{ApplyWhen, LocalFlags, Termios};

use crate::Tty;

const CS8: u32 = 0o60;
const VINTR: usize = 0;
const VERASE: usize = 2;
const VMIN: usize = 6;

impl Tty {
    pub(super) fn ioctl(&self, request: IoctlRequest<'_>) -> Result<(), IoctlError> {
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
                    .map(ProcessGroupId::as_u64)
                    .unwrap_or(0)
                    .try_into()
                    .map_err(|_| IoctlError::Invalid)?;
                Ok(())
            }
            IoctlRequest::SetForegroundPgid(pgid) => {
                let pgid = if pgid == 0 {
                    None
                } else {
                    Some(ProcessGroupId::new(u64::from(pgid)).ok_or(IoctlError::Invalid)?)
                };

                *self.foreground_pgid.lock() = pgid;
                Ok(())
            }
            IoctlRequest::FbGetVarInfo(_)
            | IoctlRequest::FbSetVarInfo(_)
            | IoctlRequest::FbGetFixedInfo(_) => Err(IoctlError::NotTty),
        }
    }

    fn termios(&self) -> Termios {
        let settings = self.line_discipline.lock().settings;

        termios_from_settings(settings)
    }

    fn set_termios(&self, when: ApplyWhen, termios: Termios) -> Result<(), IoctlError> {
        validate_termios(&termios)?;

        let _read_guard = self.read_lock.lock();

        if when == ApplyWhen::Flush {
            self.buffered.lock().clear();
            // Draining the pending queue must disable interrupts: the IRQ path pushes into the
            // same queue, and it must not run while this thread holds the queue lock.
            CurrentArchitectureBackend::without_interrupts(|| {
                while self.pending.lock().pop_front().is_some() {}
            });
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
}

fn termios_from_settings(settings: LineDisciplineSettings) -> Termios {
    let mut control_characters = [0; 32];
    control_characters[VINTR] = settings.intr_character;
    control_characters[VERASE] = settings.erase_character;
    control_characters[VMIN] = 1;

    Termios {
        input_flags: 0,
        output_flags: 0,
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
fn validate_termios(termios: &Termios) -> Result<(), IoctlError> {
    validate_fixed("ioctl.tcsetattr.input-flags", termios.input_flags, 0)?;
    validate_fixed("ioctl.tcsetattr.output-flags", termios.output_flags, 0)?;
    validate_fixed("ioctl.tcsetattr.control-flags", termios.control_flags, CS8)?;

    let supported_local = LocalFlags::ECHO | LocalFlags::ICANON | LocalFlags::ISIG;
    let unsupported_local = termios.local_flags.difference(supported_local);
    validate_fixed("ioctl.tcsetattr.local-flags", unsupported_local.bits(), 0)?;
    validate_fixed(
        "ioctl.tcsetattr.line-discipline",
        u32::from(termios.line_discipline),
        0,
    )?;
    validate_fixed("ioctl.tcsetattr.input-speed", termios.input_speed, 0)?;
    validate_fixed("ioctl.tcsetattr.output-speed", termios.output_speed, 0)?;

    let mut expected = [0; 32];
    expected[VINTR] = 0o3;
    expected[VERASE] = termios.control_characters[VERASE];
    expected[VMIN] = 1;

    if let Some((index, value)) = termios
        .control_characters
        .iter()
        .copied()
        .enumerate()
        .find(|(index, value)| *value != expected[*index])
    {
        let argument = u64::try_from(index).unwrap() << 8 | u64::from(value);

        return Err(IoctlError::Unsupported {
            operation: "ioctl.tcsetattr.control-character",
            argument,
        });
    }

    Ok(())
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
    use roxy_input::{KeyCode, KeyState};
    use roxy_test::kernel_test;
    use roxy_tty_types::{ApplyWhen, LocalFlags, Termios, WindowSize};

    use crate::test_support::{key, open};

    fn press(code: KeyCode) -> roxy_input::KeyEvent {
        key(code, KeyState::Pressed)
    }

    kernel_test!("roxy-tty::termios-ioctl", updates_input_mode, {
        let (_tty, output, file) = open(alloc::vec![press(KeyCode::X)]);
        let mut termios = Termios::default();

        file.ioctl(IoctlRequest::GetTermios(&mut termios)).unwrap();

        termios.local_flags = LocalFlags::empty();
        assert_eq!(
            file.ioctl(IoctlRequest::SetTermios {
                when: ApplyWhen::Immediate,
                termios,
            }),
            Ok(())
        );

        let mut input = [0; 1];
        assert_eq!(file.read(&mut input), Ok(1));
        assert_eq!(&input, b"x");
        assert!(output.bytes().is_empty());
    });

    kernel_test!("roxy-tty::winsize-ioctl", round_trips_window_size, {
        let (_tty, _output, file) = open(alloc::vec![]);
        let size = WindowSize {
            rows: 40,
            columns: 120,
            pixel_width: 960,
            pixel_height: 640,
        };

        assert_eq!(file.ioctl(IoctlRequest::SetWindowSize(size)), Ok(()));

        let mut returned = WindowSize::default();
        assert_eq!(
            file.ioctl(IoctlRequest::GetWindowSize(&mut returned)),
            Ok(())
        );
        assert_eq!(returned, size);
    });

    kernel_test!("roxy-tty::termios-flush", discards_pending_input, {
        let (tty, _output, file) = open(alloc::vec![press(KeyCode::X)]);
        let _ = tty.line_discipline.lock().process(b"partial");
        tty.buffered.lock().extend_from_slice(b"ready");
        let mut termios = Termios::default();

        file.ioctl(IoctlRequest::GetTermios(&mut termios)).unwrap();
        file.ioctl(IoctlRequest::SetTermios {
            when: ApplyWhen::Flush,
            termios,
        })
        .unwrap();

        assert!(tty.buffered.lock().is_empty());
        assert!(tty.pending.lock().is_empty());
        assert_eq!(
            tty.line_discipline.lock().process(b"\n").buffer.unwrap(),
            b"\n"
        );
    });

    kernel_test!("roxy-tty::termios-unsupported", rejects_input_flags, {
        let (_tty, _output, file) = open(alloc::vec![]);
        let mut termios = Termios::default();

        file.ioctl(IoctlRequest::GetTermios(&mut termios)).unwrap();
        termios.input_flags = 1;

        assert_eq!(
            file.ioctl(IoctlRequest::SetTermios {
                when: ApplyWhen::Immediate,
                termios,
            }),
            Err(IoctlError::Unsupported {
                operation: "ioctl.tcsetattr.input-flags",
                argument: 1,
            })
        );
    });

    kernel_test!(
        "roxy-tty::termios-local-flags",
        rejects_unknown_local_flags,
        {
            let (_tty, _output, file) = open(alloc::vec![]);
            let mut termios = Termios::default();

            file.ioctl(IoctlRequest::GetTermios(&mut termios)).unwrap();
            termios.local_flags = LocalFlags::from_bits_retain(0o100);

            assert_eq!(
                file.ioctl(IoctlRequest::SetTermios {
                    when: ApplyWhen::Immediate,
                    termios,
                }),
                Err(IoctlError::Unsupported {
                    operation: "ioctl.tcsetattr.local-flags",
                    argument: 0o100,
                })
            );
        }
    );

    kernel_test!("roxy-tty::termios-isig", round_trips_isig_flag, {
        let (tty, _output, file) = open(alloc::vec![]);
        let mut termios = Termios::default();

        file.ioctl(IoctlRequest::GetTermios(&mut termios)).unwrap();
        assert!(tty.line_discipline.lock().settings.isig);
        assert!(termios.local_flags.contains(LocalFlags::ISIG));
        assert_eq!(termios.control_characters[0], b'\x03');

        termios.local_flags.remove(LocalFlags::ISIG);
        file.ioctl(IoctlRequest::SetTermios {
            when: ApplyWhen::Immediate,
            termios,
        })
        .unwrap();
        assert!(!tty.line_discipline.lock().settings.isig);
    });
}
