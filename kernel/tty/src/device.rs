use alloc::sync::Arc;
use alloc::vec::Vec;

use roxy_devfs::{Device, DeviceRegistry};
use roxy_fd::{FileError, FileMetadata, FileType, IoctlError, IoctlRequest, PollEvents};
use roxy_poll::{PollListener, PollRegistration};

use crate::CONSOLE;
use crate::tty::{CONSOLE_NODE, Tty};

/// Stable file ID for the console terminal node within the devfs mount.
const CONSOLE_FILE_ID: u64 = 6;

/// Exposes the console terminal as a character device under `/dev/tty0`.
///
/// The console's initial process descriptors (fd 0/1/2) are `TtyFile`s handed out directly; this
/// device lets userspace reopen the same single terminal through the device filesystem. Both share
/// one `Arc<Tty>`, so the node's path (`CONSOLE_PATH`) equals the path `TtyFile::terminal_path`
/// reports — the contract `ttyname` relies on.
pub struct TtyDevice {
    tty: Arc<Tty>,
}

impl Device for TtyDevice {
    fn metadata(&self) -> FileMetadata {
        FileMetadata {
            file_id: CONSOLE_FILE_ID,
            file_type: FileType::CharacterDevice,
            permissions: 0o600,
            size: 0,
            hard_links: 1,
        }
    }

    fn is_terminal(&self) -> bool {
        true
    }

    fn terminal_path(&self) -> Option<Vec<u8>> {
        Some(Tty::terminal_path().to_vec())
    }

    fn read(&self, output: &mut [u8]) -> Result<usize, FileError> {
        self.tty.read(output)
    }

    fn write(&self, input: &[u8]) -> Result<usize, FileError> {
        self.tty.write(input)
    }

    fn poll(&self) -> PollEvents {
        self.tty.poll().unwrap_or_default()
    }

    fn register_poll_listener(&self, listener: Arc<PollListener>) -> PollRegistration {
        self.tty.register_poll_listener(listener)
    }

    fn ioctl(&self, request: IoctlRequest<'_>) -> Result<(), IoctlError> {
        self.tty.ioctl(request)
    }
}

/// Registers the console terminal as `/dev/tty0` with the shared device registry.
///
/// kernel-main calls this after initializing the console, so the terminal is present before any
/// process can reopen it by path.
///
/// # Panics
///
/// Panics when the console has not been initialized, or when another device already registered the
/// `tty0` path.
pub fn register_console_device(registry: &DeviceRegistry) {
    let console = CONSOLE
        .get()
        .expect("console must be initialized before registering its device node");

    registry
        .register(
            CONSOLE_NODE,
            Arc::new(TtyDevice {
                tty: console.tty.clone(),
            }),
        )
        .expect("console is registered exactly once");
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_devfs::Device;
    use roxy_fd::FileType;
    use roxy_test::kernel_test;

    use crate::device::TtyDevice;
    use crate::test_support::open;

    kernel_test!("roxy-tty::device-adapter", exposes_console_as_terminal, {
        let (tty, _output, _file) = open(alloc::vec![]);
        let device = TtyDevice { tty };

        assert!(device.is_terminal());
        assert_eq!(device.terminal_path().unwrap(), b"/dev/tty0".to_vec());
        assert_eq!(device.metadata().file_type, FileType::CharacterDevice);
    });
}
