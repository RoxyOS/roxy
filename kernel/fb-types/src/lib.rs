#![no_std]

/// Describes one color channel's bit placement inside a packed pixel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FbBitfield {
    pub offset: u32,
    pub length: u32,
}

/// Layout-neutral variable screen configuration reported by a framebuffer device.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FbVarInfo {
    pub xres: u32,
    pub yres: u32,
    pub xres_virtual: u32,
    pub yres_virtual: u32,
    pub bits_per_pixel: u32,
    pub red: FbBitfield,
    pub green: FbBitfield,
    pub blue: FbBitfield,
}

/// Layout-neutral fixed screen configuration reported by a framebuffer device.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FbFixedInfo {
    pub id: [u8; 16],
    pub smem_start: u64,
    pub smem_len: u32,
    pub visual: u32,
    pub line_length: u32,
}
