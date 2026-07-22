use core::convert::Infallible;

use embedded_bitmap_fonts::terminus::FONT_8x16;
use embedded_graphics::{
    Pixel,
    draw_target::DrawTarget,
    geometry::{Dimensions, Point, Size},
    pixelcolor::BinaryColor,
    primitives::Rectangle,
};

use crate::{InitError, framebuffer::Framebuffer};

pub(crate) const GLYPH_WIDTH: usize = 8;
pub(crate) const GLYPH_HEIGHT: usize = 16;

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

        let glyph_index = FONT_8x16
            .glyph_mapping
            .index(char::from(byte))
            .try_into()
            .expect("Terminus glyph index exceeds u32");
        let mut target = GlyphTarget {
            framebuffer: &mut self.framebuffer,
            left: column * GLYPH_WIDTH,
            top: row * GLYPH_HEIGHT,
            foreground: self.foreground,
        };

        if let Err(infallible) =
            FONT_8x16.draw_glyph(glyph_index, &mut target, BinaryColor::On, Point::zero())
        {
            match infallible {}
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

    /// Removes the cursor when this cell has one, or draws it when it is absent.
    pub(crate) fn toggle_cursor(&mut self, column: usize, row: usize) {
        assert!(column < self.columns && row < self.rows);
        self.framebuffer.xor_rect(
            column * GLYPH_WIDTH,
            row * GLYPH_HEIGHT,
            GLYPH_WIDTH,
            GLYPH_HEIGHT,
            self.foreground ^ self.background,
        );
    }

    pub(crate) fn scroll_line(&mut self) {
        let text_height = self.rows * GLYPH_HEIGHT;

        self.framebuffer
            .scroll_rows_up(GLYPH_HEIGHT, text_height, self.background);
    }
}

struct GlyphTarget<'a> {
    framebuffer: &'a mut Framebuffer,
    left: usize,
    top: usize,
    foreground: u32,
}

impl Dimensions for GlyphTarget<'_> {
    fn bounding_box(&self) -> Rectangle {
        Rectangle::new(Point::zero(), Size::new(8, 16))
    }
}

impl DrawTarget for GlyphTarget<'_> {
    type Color = BinaryColor;
    type Error = Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(point, color) in pixels {
            let Ok(glyph_x) = usize::try_from(point.x) else {
                continue;
            };
            let Ok(glyph_y) = usize::try_from(point.y) else {
                continue;
            };

            if color == BinaryColor::On && glyph_x < GLYPH_WIDTH && glyph_y < GLYPH_HEIGHT {
                self.framebuffer.write_pixel(
                    self.left + glyph_x,
                    self.top + glyph_y,
                    self.foreground,
                );
            }
        }

        Ok(())
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
