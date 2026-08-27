use roxy_fb_types::{FbBitfield, FbFixedInfo, FbVarInfo};
use roxy_fbterm::{ColorChannelLayout, FramebufferLayout};

const FB_IDENTIFIER: [u8; 16] = *b"ROXY\0\0\0\0\0\0\0\0\0\0\0\0";
const FB_VISUAL_TRUECOLOR: u32 = 2;

/// Converts the validated framebuffer layout into the variable screen info reported to
/// userspace. Timing, margin, activation, and reserved fields are zero because the device has
/// no mode-setting or panning state.
pub(super) fn var_info(layout: &FramebufferLayout) -> FbVarInfo {
    FbVarInfo {
        xres: layout.width,
        yres: layout.height,
        xres_virtual: layout.width,
        yres_virtual: layout.height,
        bits_per_pixel: layout.bits_per_pixel,
        red: channel(layout.red),
        green: channel(layout.green),
        blue: channel(layout.blue),
    }
}

/// Converts the validated framebuffer layout into the fixed screen info reported to userspace.
pub(super) fn fixed_info(layout: &FramebufferLayout) -> FbFixedInfo {
    FbFixedInfo {
        id: FB_IDENTIFIER,
        smem_start: layout.address,
        smem_len: memory_length(layout),
        visual: FB_VISUAL_TRUECOLOR,
        line_length: layout.pitch,
    }
}

/// Returns the byte length of one visible framebuffer frame.
pub(super) fn memory_length(layout: &FramebufferLayout) -> u32 {
    let length = u64::from(layout.pitch)
        .checked_mul(u64::from(layout.height))
        .expect("validated framebuffer memory length fits u64");

    u32::try_from(length).expect("validated framebuffer memory length fits u32")
}

fn channel(layout: ColorChannelLayout) -> FbBitfield {
    FbBitfield {
        offset: u32::from(layout.shift),
        length: u32::from(layout.size),
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_fb_types::FbBitfield;
    use roxy_fbterm::{ColorChannelLayout, FramebufferLayout};
    use roxy_test::kernel_test;

    use super::{fixed_info, memory_length, var_info};

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

    kernel_test!(
        "roxy-fbdev::var-conversion",
        reports_variable_screen_info,
        {
            let info = var_info(&LAYOUT);
            assert_eq!(info.xres, 1024);
            assert_eq!(info.yres, 768);
            assert_eq!(info.xres_virtual, 1024);
            assert_eq!(info.bits_per_pixel, 32);
            assert_eq!(
                info.red,
                FbBitfield {
                    offset: 16,
                    length: 8
                }
            );
            assert_eq!(
                info.green,
                FbBitfield {
                    offset: 8,
                    length: 8
                }
            );
            assert_eq!(
                info.blue,
                FbBitfield {
                    offset: 0,
                    length: 8
                }
            );
        }
    );

    kernel_test!("roxy-fbdev::fixed-conversion", reports_fixed_screen_info, {
        let info = fixed_info(&LAYOUT);
        assert_eq!(&info.id, b"ROXY\0\0\0\0\0\0\0\0\0\0\0\0");
        assert_eq!(info.smem_start, 0x1000);
        assert_eq!(info.smem_len, 4096 * 768);
        assert_eq!(info.visual, 2);
        assert_eq!(info.line_length, 4096);
    });

    kernel_test!("roxy-fbdev::memory-length", covers_one_frame, {
        assert_eq!(memory_length(&LAYOUT), 4096 * 768);
    });
}
