use crate::color::RgbColor;

use super::{DEFAULT_BACKGROUND, DEFAULT_FOREGROUND, TextRenderer};

impl TextRenderer {
    pub(crate) fn set_foreground(&mut self, color: RgbColor) {
        self.foreground = self.framebuffer.pack_rgb(color);
    }

    pub(crate) fn set_background(&mut self, color: RgbColor) {
        self.background = self.framebuffer.pack_rgb(color);
    }

    pub(crate) fn reset_foreground(&mut self) {
        self.set_foreground(DEFAULT_FOREGROUND);
    }

    pub(crate) fn reset_background(&mut self) {
        self.set_background(DEFAULT_BACKGROUND);
    }
}
