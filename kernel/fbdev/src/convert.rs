use roxy_fb_types::{FbChannel, FbInfo};
use roxy_fbterm::{ColorChannelLayout, FramebufferLayout};

/// Converts the validated framebuffer layout into the description reported to userspace.
///
/// The layout carries no timing, margin, or panning state, so nothing else has to be reported:
/// the device cannot change modes and exposes exactly one visible frame.
pub(super) fn info(layout: &FramebufferLayout) -> FbInfo {
    FbInfo {
        width: layout.width,
        height: layout.height,
        stride: layout.pitch,
        memory_length: memory_length(layout),
        red: channel(layout.red),
        green: channel(layout.green),
        blue: channel(layout.blue),
    }
}

/// Returns the byte length of one visible framebuffer frame.
pub(super) fn memory_length(layout: &FramebufferLayout) -> u32 {
    let length = u64::from(layout.pitch)
        .checked_mul(u64::from(layout.height))
        .expect("validated framebuffer memory length fits u64");

    u32::try_from(length).expect("validated framebuffer memory length fits u32")
}

fn channel(layout: ColorChannelLayout) -> FbChannel {
    FbChannel {
        size: layout.size,
        shift: layout.shift,
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_fb_types::FbChannel;
    use roxy_fbterm::{ColorChannelLayout, FramebufferLayout};
    use roxy_test::kernel_test;

    use super::{info, memory_length};

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

    kernel_test!("roxy-fbdev::info-conversion", reports_framebuffer_info, {
        let converted = info(&LAYOUT);
        assert_eq!(converted.width, 1024);
        assert_eq!(converted.height, 768);
        assert_eq!(converted.stride, 4096);
        assert_eq!(converted.memory_length, 4096 * 768);
        assert_eq!(converted.red, FbChannel { size: 8, shift: 16 });
        assert_eq!(converted.green, FbChannel { size: 8, shift: 8 });
        assert_eq!(converted.blue, FbChannel { size: 8, shift: 0 });
    });

    kernel_test!("roxy-fbdev::memory-length", multiplies_pitch_by_height, {
        assert_eq!(memory_length(&LAYOUT), 4096 * 768);
    });
}
