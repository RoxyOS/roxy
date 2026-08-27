use roxy_devfs::Device;
use roxy_fbterm::FramebufferLayout;
use roxy_fd::{FileMetadata, FileType, IoctlError, IoctlRequest, MmapError, MmapTarget};
use roxy_memory::PAGE_SIZE;

use crate::convert;

const DEVICE_ID: u64 = 1;

/// The boot framebuffer character device exposed as `/dev/fb0`.
///
/// The device is stateless: it reports the layout published by `roxy-fbterm` and maps the
/// framebuffer's physical memory without copying or ownership transfer. The framebuffer mapping
/// lives for the kernel lifetime, so the device never releases it.
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
            IoctlRequest::FbGetVarInfo(info) => {
                *info = convert::var_info(self.layout);
                Ok(())
            }
            IoctlRequest::FbGetFixedInfo(info) => {
                *info = convert::fixed_info(self.layout);
                Ok(())
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

        if size > length {
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
        FbFixedInfo, FbVarInfo, FileType, IoctlError, IoctlRequest, MmapError, MmapTarget,
    };
    use roxy_test::kernel_test;

    use super::FramebufferDevice;

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

    fn var_info_request() -> FbVarInfo {
        FbVarInfo {
            xres: 0,
            yres: 0,
            xres_virtual: 0,
            yres_virtual: 0,
            bits_per_pixel: 0,
            red: roxy_fd::FbBitfield {
                offset: 0,
                length: 0,
            },
            green: roxy_fd::FbBitfield {
                offset: 0,
                length: 0,
            },
            blue: roxy_fd::FbBitfield {
                offset: 0,
                length: 0,
            },
        }
    }

    fn fixed_info_request() -> FbFixedInfo {
        FbFixedInfo {
            id: [0; 16],
            smem_start: 0,
            smem_len: 0,
            visual: 0,
            line_length: 0,
        }
    }

    kernel_test!("roxy-fbdev::metadata", reports_character_device, {
        let device = FramebufferDevice::new(&LAYOUT);
        let metadata = device.metadata();
        assert_eq!(metadata.file_type, FileType::CharacterDevice);
        assert_eq!(metadata.permissions, 0o600);
    });

    kernel_test!("roxy-fbdev::var-info-ioctl", dispatches_var_info, {
        let device = FramebufferDevice::new(&LAYOUT);
        let mut info = var_info_request();

        device.ioctl(IoctlRequest::FbGetVarInfo(&mut info)).unwrap();
        assert_eq!(info.xres, 1024);
        assert_eq!(info.bits_per_pixel, 32);
    });

    kernel_test!("roxy-fbdev::fixed-info-ioctl", dispatches_fixed_info, {
        let device = FramebufferDevice::new(&LAYOUT);
        let mut info = fixed_info_request();

        device
            .ioctl(IoctlRequest::FbGetFixedInfo(&mut info))
            .unwrap();
        assert_eq!(info.smem_start, 0x1000);
        assert_eq!(info.smem_len, 4096 * 768);
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
    });

    kernel_test!("roxy-fbdev::unsupported-ioctl", rejects_unknown_requests, {
        let device = FramebufferDevice::new(&LAYOUT);
        assert!(matches!(
            device.ioctl(IoctlRequest::GetWindowSize(&mut Default::default())),
            Err(IoctlError::Unsupported { .. })
        ));
    });
}
