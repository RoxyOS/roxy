#![no_std]

/// Describes one packed color channel: how many bits it occupies and where they start.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FbChannel {
    pub size: u8,
    pub shift: u8,
}

/// Layout-neutral description of a framebuffer device.
///
/// This is the representation a framebuffer device reports through its typed ioctl surface. The
/// userspace-visible record that carries it across the syscall boundary belongs to `roxy-syscall`;
/// the device nodes that serve it are `/dev/framebuffer`.
///
/// Every field describes a 32-bit-per-pixel, RGB-ordered layout: the owning terminal only
/// publishes a device for a layout it validated, so a device that exists always has one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FbInfo {
    /// Visible width in pixels.
    pub width: u32,
    /// Visible height in pixels.
    pub height: u32,
    /// Bytes per row, including any padding beyond `width * 4`.
    pub stride: u32,
    /// Byte length of one frame.
    pub memory_length: u32,
    pub red: FbChannel,
    pub green: FbChannel,
    pub blue: FbChannel,
}
