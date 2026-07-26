use roxy_tty_types::WindowSize;

use crate::{color::RgbColor, renderer::TextRenderer};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EraseMode {
    ToEnd,
    ToStart,
    All,
}

impl TryFrom<u16> for EraseMode {
    type Error = u16;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::ToEnd),
            1 => Ok(Self::ToStart),
            2 => Ok(Self::All),
            _ => Err(value),
        }
    }
}

pub(crate) struct Screen {
    renderer: TextRenderer,
    column: usize,
    row: usize,
    saved_column: usize,
    saved_row: usize,
    cursor_visible: bool,
}

impl Screen {
    pub(crate) fn new(mut renderer: TextRenderer) -> Self {
        renderer.toggle_cursor(0, 0);

        Self {
            renderer,
            column: 0,
            row: 0,
            saved_column: 0,
            saved_row: 0,
            cursor_visible: true,
        }
    }

    pub(crate) fn begin_update(&mut self) {
        if self.cursor_visible {
            self.toggle_cursor();
        }
    }

    pub(crate) fn finish_update(&mut self) {
        if self.cursor_visible {
            self.toggle_cursor();
        }
    }

    pub(crate) fn columns(&self) -> usize {
        self.renderer.columns()
    }

    pub(crate) fn rows(&self) -> usize {
        self.renderer.rows()
    }

    pub(crate) fn pixel_width(&self) -> usize {
        self.renderer.pixel_width()
    }

    pub(crate) fn pixel_height(&self) -> usize {
        self.renderer.pixel_height()
    }

    pub(crate) fn window_size(&self) -> WindowSize {
        WindowSize {
            rows: saturating_u16(self.rows()),
            columns: saturating_u16(self.columns()),
            pixel_width: saturating_u16(self.pixel_width()),
            pixel_height: saturating_u16(self.pixel_height()),
        }
    }

    pub(crate) fn column(&self) -> usize {
        self.column
    }

    pub(crate) fn row(&self) -> usize {
        self.row
    }

    pub(crate) fn print(&mut self, character: char) {
        if character.is_ascii() && !character.is_ascii_control() {
            self.put(character as u8);
        }
    }

    pub(crate) fn execute(&mut self, byte: u8) {
        match byte {
            b'\n' => self.newline(),
            b'\r' => self.column = 0,
            8 => self.backspace(),
            b'\t' => self.tab(),
            _ => {}
        }
    }

    pub(crate) fn move_relative(&mut self, columns: isize, rows: isize) {
        self.column = offset(self.column, columns, self.columns());
        self.row = offset(self.row, rows, self.rows());
    }

    pub(crate) fn set_position(&mut self, column: usize, row: usize) {
        self.column = column.min(self.columns() - 1);
        self.row = row.min(self.rows() - 1);
    }

    pub(crate) fn save_cursor(&mut self) {
        self.saved_column = self.column;
        self.saved_row = self.row;
    }

    pub(crate) fn restore_cursor(&mut self) {
        self.set_position(self.saved_column, self.saved_row);
    }

    pub(crate) fn set_cursor_visible(&mut self, visible: bool) {
        self.cursor_visible = visible;
    }

    pub(crate) fn erase_display(&mut self, mode: EraseMode) {
        match mode {
            EraseMode::ToEnd => {
                self.renderer
                    .clear_cells(self.row, self.column, self.columns());
                self.renderer.clear_rows(self.row + 1, self.rows());
            }
            EraseMode::ToStart => {
                self.renderer.clear_rows(0, self.row);
                self.renderer.clear_cells(self.row, 0, self.column + 1);
            }
            EraseMode::All => self.renderer.clear_rows(0, self.rows()),
        }
    }

    pub(crate) fn erase_line(&mut self, mode: EraseMode) {
        match mode {
            EraseMode::ToEnd => self
                .renderer
                .clear_cells(self.row, self.column, self.columns()),
            EraseMode::ToStart => self.renderer.clear_cells(self.row, 0, self.column + 1),
            EraseMode::All => self.renderer.clear_cells(self.row, 0, self.columns()),
        }
    }

    pub(crate) fn set_foreground(&mut self, color: RgbColor) {
        self.renderer.set_foreground(color);
    }

    pub(crate) fn set_background(&mut self, color: RgbColor) {
        self.renderer.set_background(color);
    }

    pub(crate) fn reset_foreground(&mut self) {
        self.renderer.reset_foreground();
    }

    pub(crate) fn reset_background(&mut self) {
        self.renderer.reset_background();
    }

    fn toggle_cursor(&mut self) {
        self.renderer.toggle_cursor(self.column, self.row);
    }

    fn put(&mut self, byte: u8) {
        self.renderer.draw_ascii(self.column, self.row, byte);
        self.column += 1;

        if self.column == self.columns() {
            self.newline();
        }
    }

    fn newline(&mut self) {
        self.column = 0;
        self.row += 1;

        if self.row == self.rows() {
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

fn offset(position: usize, amount: isize, bound: usize) -> usize {
    position.saturating_add_signed(amount).min(bound - 1)
}

fn saturating_u16(value: usize) -> u16 {
    u16::try_from(value).unwrap_or(u16::MAX)
}
