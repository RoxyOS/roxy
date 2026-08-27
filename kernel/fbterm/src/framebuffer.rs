use core::ptr;

use roxy_boot::FramebufferInfo;

use crate::{InitError, color::RgbColor};

const RGB_MEMORY_MODEL: u8 = 1;
const BYTES_PER_PIXEL: usize = 4;

struct PixelFormat {
    red_size: u8,
    red_shift: u8,
    green_size: u8,
    green_shift: u8,
    blue_size: u8,
    blue_shift: u8,
}

/// Describes one color channel's bit placement inside a packed pixel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ColorChannelLayout {
    pub size: u8,
    pub shift: u8,
}

/// Kernel-side description of the validated framebuffer layout.
///
/// This is the neutral contract between `roxy-fbterm` (the layout owner) and device drivers such
/// as `roxy-fbdev` that expose the framebuffer to userspace.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FramebufferLayout {
    pub address: u64,
    pub width: u32,
    pub height: u32,
    pub pitch: u32,
    pub bits_per_pixel: u32,
    pub red: ColorChannelLayout,
    pub green: ColorChannelLayout,
    pub blue: ColorChannelLayout,
}

pub(crate) struct Framebuffer {
    address: *mut u32,
    width: usize,
    height: usize,
    pitch: usize,
    format: PixelFormat,
}

// SAFETY: Framebuffer exclusively owns access to a boot-provided mapping that remains valid for
// the kernel lifetime. Its safe methods validate coordinates before accessing the mapping.
unsafe impl Send for Framebuffer {}

impl Framebuffer {
    pub(crate) fn from_info(info: &FramebufferInfo) -> Result<Self, InitError> {
        let address = usize::try_from(info.address).map_err(|_| InitError::InvalidLayout)?;
        let width = usize::try_from(info.width).map_err(|_| InitError::InvalidLayout)?;
        let height = usize::try_from(info.height).map_err(|_| InitError::InvalidLayout)?;
        let pitch = usize::try_from(info.pitch).map_err(|_| InitError::InvalidLayout)?;
        let row_bytes = width
            .checked_mul(BYTES_PER_PIXEL)
            .ok_or(InitError::InvalidLayout)?;
        let byte_length = pitch.checked_mul(height).ok_or(InitError::InvalidLayout)?;

        if !supported(info, address, pitch, row_bytes, byte_length) {
            return Err(InitError::UnsupportedMode);
        }

        Ok(Self {
            address: ptr::with_exposed_provenance_mut(address),
            width,
            height,
            pitch,
            format: PixelFormat::from_info(info),
        })
    }

    pub(crate) fn width(&self) -> usize {
        self.width
    }

    pub(crate) fn height(&self) -> usize {
        self.height
    }

    pub(crate) fn layout(&self) -> FramebufferLayout {
        FramebufferLayout {
            address: self.address as u64,
            width: u32::try_from(self.width).expect("validated width fits u32"),
            height: u32::try_from(self.height).expect("validated height fits u32"),
            pitch: u32::try_from(self.pitch).expect("validated pitch fits u32"),
            bits_per_pixel: u32::try_from(BYTES_PER_PIXEL * 8).expect("bpp fits u32"),
            red: ColorChannelLayout {
                size: self.format.red_size,
                shift: self.format.red_shift,
            },
            green: ColorChannelLayout {
                size: self.format.green_size,
                shift: self.format.green_shift,
            },
            blue: ColorChannelLayout {
                size: self.format.blue_size,
                shift: self.format.blue_shift,
            },
        }
    }

    pub(crate) fn pack_rgb(&self, color: RgbColor) -> u32 {
        self.format.pack(color)
    }

    pub(crate) fn clear(&mut self, color: u32) {
        self.fill_rect(0, 0, self.width, self.height, color);
    }

    pub(crate) fn fill_rect(
        &mut self,
        left: usize,
        top: usize,
        width: usize,
        height: usize,
        color: u32,
    ) {
        let right = left
            .checked_add(width)
            .expect("framebuffer rectangle overflow");
        let bottom = top
            .checked_add(height)
            .expect("framebuffer rectangle overflow");
        assert!(right <= self.width && bottom <= self.height);

        for pixel_y in top..bottom {
            for pixel_x in left..right {
                self.write_pixel(pixel_x, pixel_y, color);
            }
        }
    }

    /// Applies the XOR mask to every pixel in the rectangle.
    pub(crate) fn xor_rect(
        &mut self,
        left: usize,
        top: usize,
        width: usize,
        height: usize,
        mask: u32,
    ) {
        let right = left
            .checked_add(width)
            .expect("framebuffer rectangle overflow");
        let bottom = top
            .checked_add(height)
            .expect("framebuffer rectangle overflow");
        assert!(right <= self.width && bottom <= self.height);

        for pixel_y in top..bottom {
            for pixel_x in left..right {
                let offset = pixel_y * self.pitch + pixel_x * BYTES_PER_PIXEL;

                // SAFETY: construction validates the mapping and pixel alignment, while the
                // rectangle assertions keep this four-byte pixel inside its visible row.
                unsafe { self.write_pixel_at(offset, self.read_pixel_at(offset) ^ mask) };
            }
        }
    }

    pub(crate) fn scroll_rows_up(
        &mut self,
        pixel_rows: usize,
        region_height: usize,
        clear_color: u32,
    ) {
        assert!(pixel_rows <= region_height && region_height <= self.height);
        for destination_y in 0..region_height - pixel_rows {
            let source_y = destination_y + pixel_rows;

            for pixel_x in 0..self.width {
                let color = self.read_pixel(pixel_x, source_y);
                self.write_pixel(pixel_x, destination_y, color);
            }
        }

        self.fill_rect(
            0,
            region_height - pixel_rows,
            self.width,
            pixel_rows,
            clear_color,
        );
    }

    pub(crate) fn write_pixel(&mut self, pixel_x: usize, pixel_y: usize, color: u32) {
        assert!(pixel_x < self.width && pixel_y < self.height);
        let offset = pixel_y * self.pitch + pixel_x * BYTES_PER_PIXEL;

        // SAFETY: construction validates the mapping and pixel alignment, while the assertions
        // keep this four-byte pixel inside its visible row.
        unsafe { self.write_pixel_at(offset, color) };
    }

    fn read_pixel(&self, pixel_x: usize, pixel_y: usize) -> u32 {
        assert!(pixel_x < self.width && pixel_y < self.height);
        let offset = pixel_y * self.pitch + pixel_x * BYTES_PER_PIXEL;

        // SAFETY: construction validates the mapping and pixel alignment, while the assertions
        // keep this four-byte pixel inside its visible row.
        unsafe { self.read_pixel_at(offset) }
    }

    unsafe fn read_pixel_at(&self, offset: usize) -> u32 {
        // SAFETY: callers prove this is an aligned pixel inside the framebuffer mapping.
        unsafe { ptr::read_volatile(self.address.add(offset / BYTES_PER_PIXEL)) }
    }

    unsafe fn write_pixel_at(&mut self, offset: usize, color: u32) {
        // SAFETY: callers prove this is an aligned pixel inside the framebuffer mapping.
        unsafe { ptr::write_volatile(self.address.add(offset / BYTES_PER_PIXEL), color) };
    }
}

impl PixelFormat {
    fn from_info(info: &FramebufferInfo) -> Self {
        Self {
            red_size: info.red_mask_size,
            red_shift: info.red_mask_shift,
            green_size: info.green_mask_size,
            green_shift: info.green_mask_shift,
            blue_size: info.blue_mask_size,
            blue_shift: info.blue_mask_shift,
        }
    }

    fn pack(&self, color: RgbColor) -> u32 {
        let red = scale(color.red, self.red_size) << self.red_shift;
        let green = scale(color.green, self.green_size) << self.green_shift;
        let blue = scale(color.blue, self.blue_size) << self.blue_shift;

        red | green | blue
    }
}

fn supported(
    info: &FramebufferInfo,
    address: usize,
    pitch: usize,
    row_bytes: usize,
    byte_length: usize,
) -> bool {
    info.memory_model == RGB_MEMORY_MODEL
        && info.bits_per_pixel == 32
        && pitch >= row_bytes
        && address != 0
        && address.is_multiple_of(BYTES_PER_PIXEL)
        && pitch.is_multiple_of(BYTES_PER_PIXEL)
        && valid_channel(info.red_mask_size, info.red_mask_shift)
        && valid_channel(info.green_mask_size, info.green_mask_shift)
        && valid_channel(info.blue_mask_size, info.blue_mask_shift)
        && address.checked_add(byte_length).is_some()
}

fn valid_channel(size: u8, shift: u8) -> bool {
    size > 0 && size <= 8 && u16::from(size) + u16::from(shift) <= 32
}

fn scale(value: u8, bits: u8) -> u32 {
    let maximum = (1u32 << bits) - 1;
    u32::from(value) * maximum / 255
}

#[cfg(feature = "kernel-test")]
mod tests {
    use alloc::vec;

    use roxy_boot::FramebufferInfo;
    use roxy_test::kernel_test;

    use super::Framebuffer;
    use crate::{InitError, color::RgbColor};

    fn info(address: u64) -> FramebufferInfo {
        FramebufferInfo {
            address,
            width: 16,
            height: 16,
            pitch: 64,
            bits_per_pixel: 32,
            memory_model: 1,
            red_mask_size: 8,
            red_mask_shift: 16,
            green_mask_size: 8,
            green_mask_shift: 8,
            blue_mask_size: 8,
            blue_mask_shift: 0,
        }
    }

    kernel_test!("roxy-fbterm::validate-mode", rejects_non_rgb32, {
        let mut framebuffer = info(1);
        framebuffer.bits_per_pixel = 24;

        assert!(matches!(
            Framebuffer::from_info(&framebuffer),
            Err(InitError::UnsupportedMode)
        ));
    });

    kernel_test!("roxy-fbterm::validate-pitch", rejects_short_pitch, {
        let mut framebuffer = info(1);
        framebuffer.pitch = 32;

        assert!(matches!(
            Framebuffer::from_info(&framebuffer),
            Err(InitError::UnsupportedMode)
        ));
    });

    kernel_test!("roxy-fbterm::validate-mask", rejects_invalid_color_mask, {
        let mut framebuffer = info(1);
        framebuffer.red_mask_shift = 31;

        assert!(matches!(
            Framebuffer::from_info(&framebuffer),
            Err(InitError::UnsupportedMode)
        ));
    });

    kernel_test!("roxy-fbterm::validate-range", rejects_address_overflow, {
        let framebuffer = info(u64::MAX);

        assert!(matches!(
            Framebuffer::from_info(&framebuffer),
            Err(InitError::UnsupportedMode)
        ));
    });

    kernel_test!("roxy-fbterm::pixel-format", packs_rgb_channels, {
        let mut storage = vec![0u8; 16 * 64];
        let framebuffer = Framebuffer::from_info(&info(storage.as_mut_ptr() as u64)).unwrap();

        assert_eq!(
            framebuffer.pack_rgb(RgbColor::new(0x12, 0x34, 0x56)),
            0x0012_3456
        );
    });
}
