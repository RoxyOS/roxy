use alloc::{sync::Arc, vec::Vec};

use roxy_devfs::{Device, DynamicDeviceResolver};
use roxy_fd::{FileError, FileMetadata, FileType, IoctlError, IoctlRequest, PollEvents};
use roxy_poll::{PollListener, PollRegistration};

use crate::TtyCore;

/// Stable file ID for the `/dev/tty` control-terminal node within the devfs mount.
const CONTROLLING_FILE_ID: u64 = 7;

/// A device representing `/dev/tty`: the controlling terminal of the calling process.
///
/// `ControllingTerminalResolver` produces one per open, wrapping the resolved terminal core, so all
/// file operations forward to the process's actual controlling terminal. The node itself never
/// acquires a session; it only resolves an already-established controlling terminal.
pub struct ControlTerminal {
    core: Arc<TtyCore>,
}

impl ControlTerminal {
    #[must_use]
    pub(crate) fn new(core: Arc<TtyCore>) -> Self {
        Self { core }
    }
}

impl Device for ControlTerminal {
    fn metadata(&self) -> FileMetadata {
        FileMetadata {
            file_id: CONTROLLING_FILE_ID,
            file_type: FileType::CharacterDevice,
            permissions: 0o600,
            size: 0,
            hard_links: 1,
        }
    }

    fn is_terminal(&self) -> bool {
        true
    }

    // TODO(tty-path): reporting the underlying terminal's own path (/dev/tty0, /dev/pts/N) needs
    // the TtyCore to know it; until then ttyname() on a /dev/tty fd returns nothing.
    fn terminal_path(&self) -> Option<Vec<u8>> {
        None
    }

    fn read(&self, output: &mut [u8]) -> Result<usize, FileError> {
        self.core.read(output)
    }

    fn write(&self, input: &[u8]) -> Result<usize, FileError> {
        self.core.write(input)
    }

    fn poll(&self) -> PollEvents {
        self.core.poll().unwrap_or_default()
    }

    fn register_poll_listener(&self, listener: Arc<PollListener>) -> PollRegistration {
        self.core.register_poll_listener(listener)
    }

    fn ioctl(&self, request: IoctlRequest<'_>) -> Result<(), IoctlError> {
        self.core.ioctl(request)
    }
}

/// A dynamic devfs resolver that serves the fixed `/dev/tty` node.
///
/// It resolves `tty` to the calling process's controlling terminal, or nothing when the process has
/// no controlling terminal (so the open fails rather than returning a dangling device). Non-`tty`
/// paths are ignored so this resolver never shadows the pty or other dynamic namespaces.
pub struct ControllingTerminalResolver;

impl DynamicDeviceResolver for ControllingTerminalResolver {
    fn resolve(&self, path: &[u8]) -> Option<Arc<dyn Device>> {
        if path != b"tty" {
            return None;
        }

        let session = roxy_process::current_process_session_id()?;
        let core = crate::controlling_terminal_of(session)?;

        Some(Arc::new(ControlTerminal::new(core)))
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_devfs::Device;
    use roxy_fd::{FileType, IoctlRequest};
    use roxy_tty_types::WindowSize;

    use super::ControlTerminal;
    use crate::test_support::open;

    roxy_test::kernel_test!(
        "roxy-tty-core::control-terminal-device",
        delegates_terminal_ops,
        {
            let (core, source, output) = open();
            source.push(b"hi\n");
            let device = ControlTerminal::new(core);

            assert!(device.is_terminal());
            assert_eq!(device.metadata().file_type, FileType::CharacterDevice);
            let mut buffer = [0; 8];
            assert_eq!(device.read(&mut buffer), Ok(3));
            assert_eq!(&buffer[..3], b"hi\n");
            assert_eq!(device.write(b"out"), Ok(3));
            assert_eq!(output.bytes(), b"hi\nout");
            assert_eq!(
                device.ioctl(IoctlRequest::GetWindowSize(&mut WindowSize::default())),
                Ok(())
            );
        }
    );
}
