use crate::{InitError, color::RgbColor, framebuffer::Framebuffer};

mod draw;
mod glyph;
mod style;

pub(crate) const GLYPH_WIDTH: usize = 8;
pub(crate) const GLYPH_HEIGHT: usize = 16;
const DEFAULT_FOREGROUND: RgbColor = RgbColor::WHITE;
const DEFAULT_BACKGROUND: RgbColor = RgbColor::BLACK;

pub(crate) struct TextRenderer {
    framebuffer: Framebuffer,
    columns: usize,
    rows: usize,
    foreground: u32,
    background: u32,
    cursor_mask: u32,
}

impl TextRenderer {
    pub(crate) fn new(mut framebuffer: Framebuffer) -> Result<Self, InitError> {
        let columns = framebuffer.width() / GLYPH_WIDTH;
        let rows = framebuffer.height() / GLYPH_HEIGHT;

        if columns == 0 || rows == 0 {
            return Err(InitError::UnsupportedMode);
        }

        let foreground = framebuffer.pack_rgb(DEFAULT_FOREGROUND);
        let background = framebuffer.pack_rgb(DEFAULT_BACKGROUND);
        let cursor_mask = foreground;

        framebuffer.clear(background);

        Ok(Self {
            framebuffer,
            columns,
            rows,
            foreground,
            background,
            cursor_mask,
        })
    }

    pub(crate) fn columns(&self) -> usize {
        self.columns
    }

    pub(crate) fn rows(&self) -> usize {
        self.rows
    }

    pub(crate) fn pixel_width(&self) -> usize {
        self.framebuffer.width()
    }

    pub(crate) fn pixel_height(&self) -> usize {
        self.framebuffer.height()
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use alloc::vec;

    use roxy_boot::FramebufferInfo;
    use roxy_test::kernel_test;

    use super::TextRenderer;
    use crate::framebuffer::Framebuffer;

    fn info(address: u64) -> FramebufferInfo {
        FramebufferInfo {
            address,
            width: 16,
            height: 32,
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

    kernel_test!("roxy-fbterm::draw-glyph", draws_only_target_cell, {
        let mut storage = vec![0u8; 32 * 64];
        let framebuffer = Framebuffer::from_info(&info(storage.as_mut_ptr() as u64)).unwrap();
        let mut renderer = TextRenderer::new(framebuffer).unwrap();

        renderer.draw_ascii(1, 0, b'A');

        assert!((0..16).all(|row| {
            storage[row * 64..row * 64 + 8 * 4]
                .iter()
                .all(|byte| *byte == 0)
        }));
        assert!((0..16).any(|row| {
            storage[row * 64 + 8 * 4..row * 64 + 16 * 4]
                .iter()
                .any(|byte| *byte != 0)
        }));
        assert!(storage[16 * 64..].iter().all(|byte| *byte == 0));
    });

    kernel_test!("roxy-fbterm::scroll", moves_last_row_up, {
        let mut storage = vec![0u8; 32 * 64];
        let framebuffer = Framebuffer::from_info(&info(storage.as_mut_ptr() as u64)).unwrap();
        let mut renderer = TextRenderer::new(framebuffer).unwrap();

        renderer.draw_ascii(0, 1, b'A');
        renderer.scroll_line();

        assert!(storage[..16 * 64].iter().any(|byte| *byte != 0));
        assert!(storage[16 * 64..].iter().all(|byte| *byte == 0));
    });

    kernel_test!("roxy-fbterm::minimum-size", rejects_incomplete_cell, {
        let mut storage = vec![0u8; 15 * 64];
        let mut framebuffer_info = info(storage.as_mut_ptr() as u64);
        framebuffer_info.height = 15;

        let framebuffer = Framebuffer::from_info(&framebuffer_info).unwrap();

        assert!(TextRenderer::new(framebuffer).is_err());
    });
}
