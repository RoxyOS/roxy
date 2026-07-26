use vte::Parser;

use roxy_tty_types::WindowSize;

use crate::{renderer::TextRenderer, screen::Screen};

pub(crate) struct Console {
    // OSC is unsupported, so its raw payload does not need parser storage.
    parser: Parser<0>,
    screen: Screen,
}

impl Console {
    pub(crate) fn new(renderer: TextRenderer) -> Self {
        Self {
            parser: Parser::new_with_size(),
            screen: Screen::new(renderer),
        }
    }

    pub(crate) fn write(&mut self, input: &[u8]) {
        if input.is_empty() {
            return;
        }

        self.screen.begin_update();
        self.parser.advance(&mut self.screen, input);
        self.screen.finish_update();
    }

    pub(crate) fn window_size(&self) -> WindowSize {
        self.screen.window_size()
    }
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

    kernel_test!("roxy-fbterm::cursor", maintains_cursor, {
        let mut storage = vec![0u8; 32 * 64];
        let mut console = console(&mut storage, 16, 32);

        assert!(is_solid_foreground(&storage, 64, 0, 0));
        console.write(b"A");
        let glyph = cell_bytes(&storage, 64, 0, 0);
        assert!(is_solid_foreground(&storage, 64, 1, 0));
        console.write(b"\r\n");

        assert_eq!(cell_bytes(&storage, 64, 0, 0), glyph);
    });

    kernel_test!("roxy-fbterm::controls", handles_basic_controls, {
        let mut storage = vec![0u8; 32 * 64];
        let mut console = console(&mut storage, 16, 32);

        console.write(b"AB\x08\t\n");

        assert_eq!(console.screen.column(), 0);
        assert_eq!(console.screen.row(), 1);
    });

    kernel_test!("roxy-fbterm::wrap-scroll", wraps_and_scrolls, {
        let mut storage = vec![0u8; 32 * 64];
        let mut console = console(&mut storage, 16, 32);

        console.write(b"AB");
        assert_eq!((console.screen.column(), console.screen.row()), (0, 1));
        console.write(b"C\n");

        assert_eq!(console.screen.row(), 1);
        assert!(cell_bytes(&storage, 64, 0, 0).iter().any(|byte| *byte != 0));
    });

    kernel_test!("roxy-fbterm::split-csi", retains_parser_state, {
        let mut storage = vec![0u8; 32 * 64];
        let mut console = console(&mut storage, 16, 32);

        console.write(b"\x1b[");
        console.write(b"2;2H");

        assert_eq!((console.screen.column(), console.screen.row()), (1, 1));
    });

    kernel_test!("roxy-fbterm::ansi-cursor", moves_and_restores_cursor, {
        let mut storage = vec![0u8; 32 * 64];
        let mut console = console(&mut storage, 16, 32);

        console.write(b"\x1b[?25l\x1b[2;2H\x1b7\x1b[99A\x1b8");
        assert_eq!((console.screen.column(), console.screen.row()), (1, 1));
        assert!(!is_solid_foreground(&storage, 64, 1, 1));
        console.write(b"\x1b[?25h");

        assert!(is_solid_foreground(&storage, 64, 1, 1));
    });

    kernel_test!("roxy-fbterm::ansi-color", applies_and_resets_colors, {
        let mut storage = vec![0u8; 32 * 64];
        let mut console = console(&mut storage, 16, 32);

        console.write(b"\x1b[?25l\x1b[31;44mA\x1b[K");
        let first = cell_bytes(&storage, 64, 0, 0);
        let second = cell_bytes(&storage, 64, 1, 0);

        assert!(first.as_chunks::<4>().0.contains(&[0, 0, 0xaa, 0]));
        assert!(first.as_chunks::<4>().0.contains(&[0xaa, 0, 0, 0]));
        assert!(
            second
                .as_chunks::<4>()
                .0
                .iter()
                .all(|pixel| *pixel == [0xaa, 0, 0, 0])
        );

        console.write(b"\x1b[0mB");
        let second = cell_bytes(&storage, 64, 1, 0);
        assert!(second.as_chunks::<4>().0.contains(&[0xff, 0xff, 0xff, 0]));

        console.write(b"\x1b[91;104mC");
        let bright = cell_bytes(&storage, 64, 0, 1);
        assert!(bright.as_chunks::<4>().0.contains(&[0x55, 0x55, 0xff, 0]));
        assert!(bright.as_chunks::<4>().0.contains(&[0xff, 0x55, 0x55, 0]));
    });

    kernel_test!("roxy-fbterm::ansi-colored-cursor", restores_colored_cell, {
        let mut storage = vec![0u8; 32 * 64];
        let mut console = console(&mut storage, 16, 32);

        console.write(b"\x1b[?25l\x1b[31mA");
        let glyph = cell_bytes(&storage, 64, 0, 0);
        console.write(b"\x1b[H\x1b[?25h");
        console.write(b"\x1b[?25l");

        assert_eq!(cell_bytes(&storage, 64, 0, 0), glyph);
    });

    kernel_test!("roxy-fbterm::ansi-erase", erases_selected_display, {
        let mut storage = vec![0u8; 64 * 64];
        let mut console = console(&mut storage, 32, 32);

        console.write(b"\x1b[?25lAB\nCD\x1b[1J");

        assert!(storage.iter().all(|byte| *byte == 0));
    });

    kernel_test!(
        "roxy-fbterm::ansi-erase-modes",
        handles_remaining_erase_modes,
        {
            let mut storage = vec![0u8; 32 * 64];
            let mut console = console(&mut storage, 16, 32);

            console.write(b"\x1b[?25lAB\x1b[H\x1b[J");
            assert!(storage.iter().all(|byte| *byte == 0));
            console.write(b"AB\x1b[1;2H\x1b[1K");
            assert!(storage.iter().all(|byte| *byte == 0));
            console.write(b"AB\x1b[H\x1b[2K");
            assert!(storage.iter().all(|byte| *byte == 0));
            console.write(b"AB\x1b[2J");
            assert!(storage.iter().all(|byte| *byte == 0));
        }
    );

    kernel_test!("roxy-fbterm::ignored-byte", ignores_non_ascii, {
        let mut storage = vec![0u8; 32 * 64];
        let mut console = console(&mut storage, 16, 32);

        console.write(&[0xff]);

        assert_eq!((console.screen.column(), console.screen.row()), (0, 0));
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
