use core::mem::{align_of, offset_of, size_of};

use roxy_fb_types::FbInfo;

use crate::{args::Out, errno::Errno};

/// Roxy `struct roxy_framebuffer_info`, the record `/dev/framebuffer` reports.
///
/// The layout is Roxy's own and mirrors `sysdeps/roxy/include/roxy/framebuffer-dev.h` in the Roxy
/// mlibc fork: four 32-bit fields, eight single-byte channel fields, and two reserved bytes, for
/// 24 bytes without padding. Pixels are always 32 bits wide, so no bit-depth field is needed.
///
/// This record only ever travels kernel-to-userspace: the device has no request that takes an
/// input record, so no decoding side exists.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RoxyFramebufferInfoAbi {
    width: u32,
    height: u32,
    stride: u32,
    memory_length: u32,
    red_size: u8,
    red_shift: u8,
    green_size: u8,
    green_shift: u8,
    blue_size: u8,
    blue_shift: u8,
    reserved0: u8,
    reserved1: u8,
}

const _: () = assert!(size_of::<RoxyFramebufferInfoAbi>() == 24);
const _: () = assert!(align_of::<RoxyFramebufferInfoAbi>() == 4);
const _: () = assert!(offset_of!(RoxyFramebufferInfoAbi, width) == 0);
const _: () = assert!(offset_of!(RoxyFramebufferInfoAbi, height) == 4);
const _: () = assert!(offset_of!(RoxyFramebufferInfoAbi, stride) == 8);
const _: () = assert!(offset_of!(RoxyFramebufferInfoAbi, memory_length) == 12);
const _: () = assert!(offset_of!(RoxyFramebufferInfoAbi, red_size) == 16);
const _: () = assert!(offset_of!(RoxyFramebufferInfoAbi, green_size) == 18);
const _: () = assert!(offset_of!(RoxyFramebufferInfoAbi, blue_size) == 20);
const _: () = assert!(offset_of!(RoxyFramebufferInfoAbi, reserved0) == 22);

impl RoxyFramebufferInfoAbi {
    /// Encodes the device-neutral description, zeroing both reserved bytes.
    const fn encode(info: FbInfo) -> Self {
        Self {
            width: info.width,
            height: info.height,
            stride: info.stride,
            memory_length: info.memory_length,
            red_size: info.red.size,
            red_shift: info.red.shift,
            green_size: info.green.size,
            green_shift: info.green.shift,
            blue_size: info.blue.size,
            blue_shift: info.blue.shift,
            reserved0: 0,
            reserved1: 0,
        }
    }
}

/// Writes the device's description into a client-provided record.
pub(super) fn write_info(output: Out<RoxyFramebufferInfoAbi>, info: FbInfo) -> Result<(), Errno> {
    let abi = RoxyFramebufferInfoAbi::encode(info);

    // SAFETY: every field is initialized and the layout has no implicit padding, so every byte of
    // the object representation is defined.
    unsafe { output.write(&abi) }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_fb_types::{FbChannel, FbInfo};
    use roxy_test::kernel_test;

    use super::RoxyFramebufferInfoAbi;

    kernel_test!(
        "roxy-syscall::framebuffer-info-encoding",
        encodes_neutral_info,
        {
            let info = FbInfo {
                width: 1280,
                height: 800,
                stride: 5120,
                memory_length: 5120 * 800,
                red: FbChannel { size: 8, shift: 16 },
                green: FbChannel { size: 8, shift: 8 },
                blue: FbChannel { size: 8, shift: 0 },
            };

            let abi = RoxyFramebufferInfoAbi::encode(info);
            assert_eq!(abi.width, 1280);
            assert_eq!(abi.height, 800);
            assert_eq!(abi.stride, 5120);
            assert_eq!(abi.memory_length, 5120 * 800);
            assert_eq!(abi.red_size, 8);
            assert_eq!(abi.red_shift, 16);
            assert_eq!(abi.green_size, 8);
            assert_eq!(abi.green_shift, 8);
            assert_eq!(abi.blue_size, 8);
            assert_eq!(abi.blue_shift, 0);
            assert_eq!(abi.reserved0, 0);
            assert_eq!(abi.reserved1, 0);
        }
    );
}
