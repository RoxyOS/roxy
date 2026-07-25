use roxy_tty_types::WindowSize;

use crate::renderer::TextRenderer;

pub(crate) struct Console {
    renderer: TextRenderer,
    column: usize,
    row: usize,
}

impl Console {
    pub(crate) fn new(mut renderer: TextRenderer) -> Self {
        renderer.toggle_cursor(0, 0);

        Self {
            renderer,
            column: 0,
            row: 0,
        }
    }

    pub(crate) fn write(&mut self, input: &[u8]) {
        if input.is_empty() {
            return;
        }

        self.toggle_cursor();

        for byte in input {
            match byte {
                b'\n' => self.newline(),
                b'\r' => self.column = 0,
                8 => self.backspace(),
                b'\t' => self.tab(),
                0x20..=0x7e => self.put(*byte),
                _ => {}
            }
        }

        self.toggle_cursor();
    }

    pub(crate) fn window_size(&self) -> WindowSize {
        WindowSize {
            rows: saturating_u16(self.renderer.rows()),
            columns: saturating_u16(self.renderer.columns()),
            pixel_width: saturating_u16(self.renderer.pixel_width()),
            pixel_height: saturating_u16(self.renderer.pixel_height()),
        }
    }

    fn toggle_cursor(&mut self) {
        self.renderer.toggle_cursor(self.column, self.row);
    }

    fn put(&mut self, byte: u8) {
        self.renderer.draw_ascii(self.column, self.row, byte);
        self.column += 1;

        if self.column == self.renderer.columns() {
            self.newline();
        }
    }

    fn newline(&mut self) {
        self.column = 0;
        self.row += 1;

        if self.row == self.renderer.rows() {
            self.renderer.scroll_line();
            self.row -= 1;
        }
    }

    fn backspace(&mut self) {
        if self.column > 0 {
            self.column -= 1;
            self.renderer.clear_cell(self.column, self.row);
        }
    }

    fn tab(&mut self) {
        let spaces = 8 - self.column % 8;

        for _ in 0..spaces {
            self.put(b' ');
        }
    }
}

fn saturating_u16(value: usize) -> u16 {
    u16::try_from(value).unwrap_or(u16::MAX)
}

#[cfg(feature = "kernel-test")]
mod tests {
    use alloc::{vec, vec::Vec};

    use roxy_boot::FramebufferInfo;
    use roxy_test::kernel_test;
    use roxy_tty_types::WindowSize;

    use super::Console;
    use crate::{
        framebuffer::Framebuffer,
        renderer::{GLYPH_HEIGHT, GLYPH_WIDTH, TextRenderer},
    };

    fn console(storage: &mut [u8], width: u64, height: u64) -> Console {
        let info = FramebufferInfo {
            address: storage.as_mut_ptr() as u64,
            width,
            height,
            pitch: width * 4,
            bits_per_pixel: 32,
            memory_model: 1,
            red_mask_size: 8,
            red_mask_shift: 16,
            green_mask_size: 8,
            green_mask_shift: 8,
            blue_mask_size: 8,
            blue_mask_shift: 0,
        };
        let framebuffer = Framebuffer::from_info(&info).unwrap();

        Console::new(TextRenderer::new(framebuffer).unwrap())
    }

    fn cell_bytes(storage: &[u8], pitch: usize, column: usize, row: usize) -> Vec<u8> {
        let left = column * GLYPH_WIDTH * 4;
        let top = row * GLYPH_HEIGHT;

        (top..top + GLYPH_HEIGHT)
            .flat_map(|pixel_row| {
                storage[pixel_row * pitch + left..pixel_row * pitch + left + 32]
                    .iter()
                    .copied()
            })
            .collect()
    }

    fn is_solid_foreground(storage: &[u8], pitch: usize, column: usize, row: usize) -> bool {
        cell_bytes(storage, pitch, column, row)
            .as_chunks::<4>()
            .0
            .iter()
            .all(|pixel| *pixel == [0xff, 0xff, 0xff, 0])
    }

    kernel_test!("roxy-fbterm::cursor-initial", shows_current_cell, {
        let mut storage = vec![0u8; 32 * 64];
        let mut console = console(&mut storage, 16, 32);

        assert!(is_solid_foreground(&storage, 64, 0, 0));
        console.write(b"A");
        assert!(!is_solid_foreground(&storage, 64, 0, 0));
        assert!(is_solid_foreground(&storage, 64, 1, 0));
    });

    kernel_test!("roxy-fbterm::cursor-restore", restores_cell_after_move, {
        let mut storage = vec![0u8; 32 * 64];
        let mut console = console(&mut storage, 16, 32);

        console.write(b"A");
        let glyph = cell_bytes(&storage, 64, 0, 0);
        console.write(b"\r\n");

        assert_eq!(cell_bytes(&storage, 64, 0, 0), glyph);
    });

    kernel_test!("roxy-fbterm::controls", updates_cursor, {
        let mut storage = vec![0u8; 32 * 64];
        let mut console = console(&mut storage, 16, 32);

        console.write(b"A\rB\x08\n");

        assert_eq!(console.column, 0);
        assert_eq!(console.row, 1);
    });

    kernel_test!("roxy-fbterm::wrap", wraps_at_last_column, {
        let mut storage = vec![0u8; 32 * 64];
        let mut console = console(&mut storage, 16, 32);

        console.write(b"AB");

        assert_eq!(console.column, 0);
        assert_eq!(console.row, 1);
    });

    kernel_test!("roxy-fbterm::scroll-overflow", scrolls_after_last_row, {
        let mut storage = vec![0u8; 32 * 64];
        let mut console = console(&mut storage, 16, 32);

        console.write(b"A\nB");
        let bottom_cell = cell_bytes(&storage, 64, 0, 1);
        console.write(b"\n");

        assert_eq!(cell_bytes(&storage, 64, 0, 0), bottom_cell);
        assert_eq!(console.column, 0);
        assert_eq!(console.row, 1);
    });

    kernel_test!("roxy-fbterm::ignored-byte", ignores_non_ascii, {
        let mut storage = vec![0u8; 32 * 64];
        let mut console = console(&mut storage, 16, 32);

        console.write(&[0xff]);

        assert_eq!(console.column, 0);
        assert_eq!(console.row, 0);
    });

    kernel_test!("roxy-fbterm::tab", advances_to_next_tab_stop, {
        let mut storage = vec![0u8; 128 * 16 * 4];
        let mut console = console(&mut storage, 128, 16);

        console.write(b"A\t");

        assert_eq!(console.column, 8);
        assert_eq!(console.row, 0);
    });

    kernel_test!("roxy-fbterm::window-size", reports_text_grid, {
        let mut storage = vec![0u8; 128 * 32 * 4];
        let console = console(&mut storage, 128, 32);

        assert_eq!(
            console.window_size(),
            WindowSize {
                rows: 2,
                columns: 16,
                pixel_width: 128,
                pixel_height: 32,
            }
        );
    });
}
