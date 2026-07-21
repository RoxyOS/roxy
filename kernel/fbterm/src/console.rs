use crate::renderer::TextRenderer;

pub(crate) struct Console {
    renderer: TextRenderer,
    column: usize,
    row: usize,
}

impl Console {
    pub(crate) fn new(renderer: TextRenderer) -> Self {
        Self {
            renderer,
            column: 0,
            row: 0,
        }
    }

    pub(crate) fn write(&mut self, input: &[u8]) {
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

#[cfg(feature = "kernel-test")]
mod tests {
    use alloc::vec;

    use roxy_boot::FramebufferInfo;
    use roxy_test::kernel_test;

    use super::Console;
    use crate::{framebuffer::Framebuffer, renderer::TextRenderer};

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
}
