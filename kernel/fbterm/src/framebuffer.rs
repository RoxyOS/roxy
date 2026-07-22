use core::{ptr, slice};

use roxy_boot::FramebufferInfo;

use crate::InitError;

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

pub(crate) struct Framebuffer {
    address: *mut u8,
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

    pub(crate) fn pack_rgb(&self, components: [u8; 3]) -> u32 {
        self.format.pack(components)
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

                // SAFETY: construction validates the full pitch * height mapping, and the
                // rectangle assertions keep this four-byte pixel inside its visible row.
                unsafe {
                    let pixel = ptr::read_unaligned(self.address.add(offset).cast::<u32>());
                    ptr::write_unaligned(self.address.add(offset).cast::<u32>(), pixel ^ mask);
                }
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
        let source_offset = self.pitch * pixel_rows;
        let copy_length = self.pitch * (region_height - pixel_rows);
        let clear_offset = copy_length;
        let clear_length = self.pitch * pixel_rows;

        // SAFETY: construction validates pitch * height, the assertions bound both regions, and
        // ptr::copy permits their overlap while moving rows toward the start of the mapping. The
        // vacated rows form the following in-bounds byte range.
        unsafe {
            ptr::copy(self.address.add(source_offset), self.address, copy_length);
            slice::from_raw_parts_mut(self.address.add(clear_offset), clear_length).fill(0);
        }

        if clear_color != 0 {
            self.fill_rect(
                0,
                region_height - pixel_rows,
                self.width,
                pixel_rows,
                clear_color,
            );
        }
    }

    pub(crate) fn write_pixel(&mut self, pixel_x: usize, pixel_y: usize, color: u32) {
        assert!(pixel_x < self.width && pixel_y < self.height);
        let offset = pixel_y * self.pitch + pixel_x * BYTES_PER_PIXEL;

        // SAFETY: construction validates the full pitch * height mapping, and the assertions keep
        // the four-byte pixel inside its visible row.
        unsafe { ptr::write_unaligned(self.address.add(offset).cast::<u32>(), color) };
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

    fn pack(&self, components: [u8; 3]) -> u32 {
        let red = scale(components[0], self.red_size) << self.red_shift;
        let green = scale(components[1], self.green_size) << self.green_shift;
        let blue = scale(components[2], self.blue_size) << self.blue_shift;

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
    use crate::InitError;

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

        assert_eq!(framebuffer.pack_rgb([0x12, 0x34, 0x56]), 0x0012_3456);
    });
}
