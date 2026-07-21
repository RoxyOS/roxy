use font8x8::legacy::BASIC_LEGACY;

use crate::{InitError, framebuffer::Framebuffer};

pub(crate) const GLYPH_WIDTH: usize = 8;
pub(crate) const GLYPH_HEIGHT: usize = 8;

pub(crate) struct TextRenderer {
    framebuffer: Framebuffer,
    columns: usize,
    rows: usize,
    foreground: u32,
    background: u32,
}

impl TextRenderer {
    pub(crate) fn new(mut framebuffer: Framebuffer) -> Result<Self, InitError> {
        let columns = framebuffer.width() / GLYPH_WIDTH;
        let rows = framebuffer.height() / GLYPH_HEIGHT;

        if columns == 0 || rows == 0 {
            return Err(InitError::UnsupportedMode);
        }

        let foreground = framebuffer.pack_rgb([0xff, 0xff, 0xff]);
        let background = framebuffer.pack_rgb([0, 0, 0]);

        framebuffer.clear(background);

        Ok(Self {
            framebuffer,
            columns,
            rows,
            foreground,
            background,
        })
    }

    pub(crate) fn columns(&self) -> usize {
        self.columns
    }

    pub(crate) fn rows(&self) -> usize {
        self.rows
    }

    pub(crate) fn draw_ascii(&mut self, column: usize, row: usize, byte: u8) {
        assert!((0x20..=0x7e).contains(&byte));
        self.clear_cell(column, row);
        let glyph = BASIC_LEGACY[usize::from(byte)];

        for (glyph_y, bits) in glyph.iter().enumerate() {
            for glyph_x in 0..GLYPH_WIDTH {
                if bits & (1 << glyph_x) != 0 {
                    self.draw_glyph_pixel(column, row, glyph_x, glyph_y);
                }
            }
        }
    }

    pub(crate) fn clear_cell(&mut self, column: usize, row: usize) {
        assert!(column < self.columns && row < self.rows);

        self.framebuffer.fill_rect(
            column * GLYPH_WIDTH,
            row * GLYPH_HEIGHT,
            GLYPH_WIDTH,
            GLYPH_HEIGHT,
            self.background,
        );
    }

    pub(crate) fn scroll_line(&mut self) {
        let text_height = self.rows * GLYPH_HEIGHT;

        self.framebuffer
            .scroll_rows_up(GLYPH_HEIGHT, text_height, self.background);
    }

    fn draw_glyph_pixel(&mut self, column: usize, row: usize, glyph_x: usize, glyph_y: usize) {
        self.framebuffer.write_pixel(
            column * GLYPH_WIDTH + glyph_x,
            row * GLYPH_HEIGHT + glyph_y,
            self.foreground,
        );
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

    kernel_test!("roxy-fbterm::draw-glyph", draws_only_target_cell, {
        let mut storage = vec![0u8; 16 * 64];
        let framebuffer = Framebuffer::from_info(&info(storage.as_mut_ptr() as u64)).unwrap();
        let mut renderer = TextRenderer::new(framebuffer).unwrap();

        renderer.draw_ascii(1, 0, b'A');

        assert!((0..8).all(|row| {
            storage[row * 64..row * 64 + 8 * 4]
                .iter()
                .all(|byte| *byte == 0)
        }));
        assert!((0..8).any(|row| {
            storage[row * 64 + 8 * 4..row * 64 + 16 * 4]
                .iter()
                .any(|byte| *byte != 0)
        }));
        assert!(storage[8 * 64..].iter().all(|byte| *byte == 0));
    });

    kernel_test!("roxy-fbterm::scroll", moves_last_row_up, {
        let mut storage = vec![0u8; 16 * 64];
        let framebuffer = Framebuffer::from_info(&info(storage.as_mut_ptr() as u64)).unwrap();
        let mut renderer = TextRenderer::new(framebuffer).unwrap();

        renderer.draw_ascii(0, 1, b'A');
        renderer.scroll_line();

        assert!(storage[..8 * 64].iter().any(|byte| *byte != 0));
        assert!(storage[8 * 64..].iter().all(|byte| *byte == 0));
    });

    kernel_test!("roxy-fbterm::minimum-size", rejects_incomplete_cell, {
        let mut storage = vec![0u8; 7 * 64];
        let mut framebuffer_info = info(storage.as_mut_ptr() as u64);
        framebuffer_info.height = 7;

        let framebuffer = Framebuffer::from_info(&framebuffer_info).unwrap();

        assert!(TextRenderer::new(framebuffer).is_err());
    });
}
