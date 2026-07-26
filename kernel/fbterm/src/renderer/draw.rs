use super::{GLYPH_HEIGHT, GLYPH_WIDTH, TextRenderer};

impl TextRenderer {
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

    pub(crate) fn clear_cells(&mut self, row: usize, start: usize, end: usize) {
        assert!(row < self.rows && start <= end && end <= self.columns);
        self.framebuffer.fill_rect(
            start * GLYPH_WIDTH,
            row * GLYPH_HEIGHT,
            (end - start) * GLYPH_WIDTH,
            GLYPH_HEIGHT,
            self.background,
        );
    }

    pub(crate) fn clear_rows(&mut self, start: usize, end: usize) {
        assert!(start <= end && end <= self.rows);
        self.framebuffer.fill_rect(
            0,
            start * GLYPH_HEIGHT,
            self.columns * GLYPH_WIDTH,
            (end - start) * GLYPH_HEIGHT,
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
            self.cursor_mask,
        );
    }

    pub(crate) fn scroll_line(&mut self) {
        let text_height = self.rows * GLYPH_HEIGHT;

        self.framebuffer
            .scroll_rows_up(GLYPH_HEIGHT, text_height, self.background);
    }
}
