use roxy_devfs::Device;
use roxy_fbterm::FramebufferLayout;
use roxy_fd::{FileMetadata, FileType, IoctlError, IoctlRequest, MmapError, MmapTarget};
use roxy_memory::PAGE_SIZE;

use crate::convert;

const DEVICE_ID: u64 = 1;

/// The boot framebuffer character device exposed as `/dev/framebuffer`.
///
/// The device reports the layout published by `roxy-fbterm`, maps the framebuffer's physical
/// memory without copying or ownership transfer, and hands the visible frame to one process at a
/// time. It holds no mutable state of its own: ownership lives in [`crate::claim`], which is
/// process-wide because there is exactly one boot framebuffer. The framebuffer mapping lives for
/// the kernel lifetime, so the device never releases it.
pub struct FramebufferDevice {
    layout: &'static FramebufferLayout,
}

impl FramebufferDevice {
    #[must_use]
    pub fn new(layout: &'static FramebufferLayout) -> Self {
        Self { layout }
    }
}

impl Device for FramebufferDevice {
    fn metadata(&self) -> FileMetadata {
        FileMetadata {
            file_id: DEVICE_ID,
            file_type: FileType::CharacterDevice,
            permissions: 0o600,
            size: 0,
            hard_links: 1,
        }
    }

    fn ioctl(&self, request: IoctlRequest<'_>) -> Result<(), IoctlError> {
        match request {
            IoctlRequest::FbGetInfo(info) => {
                *info = convert::info(self.layout);
                Ok(())
            }
            // Control is claimed for the calling process, which is the identity every other
            // request of this device is independent of: mapping stays available while another
            // process holds the frame, exactly as it is before anyone holds it.
            IoctlRequest::FbTakeControl => super::claim::take(roxy_process::current_process_id()),
            IoctlRequest::FbReleaseControl => {
                super::claim::release(roxy_process::current_process_id())
            }
            _ => Err(IoctlError::Unsupported {
                operation: "fbdev.ioctl",
                argument: 0,
            }),
        }
    }

    fn mmap(&self, size: usize, offset: u64) -> Result<MmapTarget, MmapError> {
        if offset != 0 || !self.layout.address.is_multiple_of(PAGE_SIZE) {
            return Err(MmapError::InvalidArgument);
        }

        let length =
            usize::try_from(convert::memory_length(self.layout)).expect("memory length fits usize");

        // Userspace rounds the mapping length up to whole pages, so accept sizes up to the
        // page-rounded framebuffer length even when pitch * height is not itself page-aligned.
        if size > length.next_multiple_of(usize::try_from(PAGE_SIZE).expect("page size fits usize"))
        {
            return Err(MmapError::InvalidArgument);
        }

        Ok(MmapTarget {
            physical_address: self.layout.address,
            length,
        })
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_devfs::Device;
    use roxy_fbterm::{ColorChannelLayout, FramebufferLayout};
    use roxy_fd::{
        FbChannel, FbInfo, FileType, IoctlError, IoctlRequest, MmapError, MmapTarget, WindowSize,
    };
    use roxy_process::ProcessId;
    use roxy_test::kernel_test;

    use super::FramebufferDevice;
    use crate::claim;

    const LAYOUT: FramebufferLayout = FramebufferLayout {
        address: 0x1000,
        width: 1024,
        height: 768,
        pitch: 4096,
        bits_per_pixel: 32,
        red: ColorChannelLayout { size: 8, shift: 16 },
        green: ColorChannelLayout { size: 8, shift: 8 },
        blue: ColorChannelLayout { size: 8, shift: 0 },
    };

    /// A layout whose framebuffer length is not page-aligned, for page-rounded mmap tests.
    const UNALIGNED_LENGTH_LAYOUT: FramebufferLayout = FramebufferLayout {
        address: 0x2000,
        pitch: 5464,
        height: 1,
        ..LAYOUT
    };

    fn empty_info() -> FbInfo {
        FbInfo {
            width: 0,
            height: 0,
            stride: 0,
            memory_length: 0,
            red: FbChannel { size: 0, shift: 0 },
            green: FbChannel { size: 0, shift: 0 },
            blue: FbChannel { size: 0, shift: 0 },
        }
    }

    kernel_test!("roxy-fbdev::metadata", reports_character_device, {
        let device = FramebufferDevice::new(&LAYOUT);
        let metadata = device.metadata();
        assert_eq!(metadata.file_type, FileType::CharacterDevice);
        assert_eq!(metadata.permissions, 0o600);
    });

    kernel_test!("roxy-fbdev::get-info-ioctl", dispatches_framebuffer_info, {
        let device = FramebufferDevice::new(&LAYOUT);
        let mut info = empty_info();

        device.ioctl(IoctlRequest::FbGetInfo(&mut info)).unwrap();
        assert_eq!(info.width, 1024);
        assert_eq!(info.height, 768);
        assert_eq!(info.stride, 4096);
        assert_eq!(info.red, FbChannel { size: 8, shift: 16 });
    });

    kernel_test!("roxy-fbdev::mmap", maps_framebuffer_memory, {
        let device = FramebufferDevice::new(&LAYOUT);
        assert_eq!(
            device.mmap(4096 * 768, 0).unwrap(),
            MmapTarget {
                physical_address: 0x1000,
                length: 4096 * 768
            }
        );
        assert_eq!(
            device.mmap(4096 * 768 + 1, 0),
            Err(MmapError::InvalidArgument)
        );
        assert_eq!(device.mmap(4096, 4096), Err(MmapError::InvalidArgument));

        // A framebuffer length that is not page-aligned still accepts mapping sizes rounded
        // up to whole pages, matching the page granularity of the mmap ABI.
        let unaligned = FramebufferDevice::new(&UNALIGNED_LENGTH_LAYOUT);
        assert!(unaligned.mmap(5464, 0).is_ok());
        assert!(unaligned.mmap(8192, 0).is_ok());
        assert_eq!(unaligned.mmap(8193, 0), Err(MmapError::InvalidArgument));
    });

    kernel_test!("roxy-fbdev::unsupported-ioctl", rejects_unknown_requests, {
        let device = FramebufferDevice::new(&LAYOUT);
        assert!(matches!(
            device.ioctl(IoctlRequest::GetWindowSize(&mut WindowSize::default())),
            Err(IoctlError::Unsupported { .. })
        ));
    });

    kernel_test!(
        "roxy-fbdev::framebuffer-claim",
        hands_the_frame_to_one_process,
        {
            let first = ProcessId::new(1).unwrap();
            let second = ProcessId::new(2).unwrap();

            assert_eq!(claim::take(first), Ok(()));
            // The holder can assert ownership again, while another process cannot take the frame.
            assert_eq!(claim::take(first), Ok(()));
            assert_eq!(claim::take(second), Err(IoctlError::Busy));
            assert_eq!(claim::release(second), Err(IoctlError::Invalid));

            assert_eq!(claim::release(first), Ok(()));
            assert_eq!(claim::release(first), Err(IoctlError::Invalid));
            assert_eq!(claim::take(second), Ok(()));

            // A process that exits without releasing frees the frame for the next client.
            claim::release_exited(second);
            assert_eq!(claim::release(second), Err(IoctlError::Invalid));
            assert_eq!(claim::take(first), Ok(()));
            assert_eq!(claim::release(first), Ok(()));

            // The frame is free again, so an unrelated process can still take it.
            assert_eq!(claim::take(second), Ok(()));
            claim::release_exited(second);
        }
    );
}
