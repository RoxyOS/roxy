use core::convert::Infallible;

use embedded_bitmap_fonts::terminus::FONT_8x16;
use embedded_graphics::{
    Pixel,
    draw_target::DrawTarget,
    geometry::{Dimensions, Point, Size},
    pixelcolor::BinaryColor,
    primitives::Rectangle,
};

use super::{GLYPH_HEIGHT, GLYPH_WIDTH, TextRenderer};
use crate::framebuffer::Framebuffer;

impl TextRenderer {
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
