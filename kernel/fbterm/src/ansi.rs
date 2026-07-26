use vte::{Params, Perform};

use crate::{
    color::RgbColor,
    screen::{EraseMode, Screen},
};

const ANSI_COLORS: [RgbColor; 16] = [
    RgbColor::BLACK,
    RgbColor::new(170, 0, 0),
    RgbColor::new(0, 170, 0),
    RgbColor::new(170, 85, 0),
    RgbColor::new(0, 0, 170),
    RgbColor::new(170, 0, 170),
    RgbColor::new(0, 170, 170),
    RgbColor::new(170, 170, 170),
    RgbColor::new(85, 85, 85),
    RgbColor::new(255, 85, 85),
    RgbColor::new(85, 255, 85),
    RgbColor::new(255, 255, 85),
    RgbColor::new(85, 85, 255),
    RgbColor::new(255, 85, 255),
    RgbColor::new(85, 255, 255),
    RgbColor::WHITE,
];

impl Perform for Screen {
    fn print(&mut self, character: char) {
        self.print(character);
    }

    fn execute(&mut self, byte: u8) {
        self.execute(byte);
    }

    fn csi_dispatch(&mut self, params: &Params, intermediates: &[u8], ignore: bool, action: char) {
        if ignore {
            return;
        }

        if intermediates == b"?" {
            self.private_mode(params, action);
        } else if intermediates.is_empty() {
            self.csi(params, action);
        }
    }

    fn esc_dispatch(&mut self, intermediates: &[u8], ignore: bool, byte: u8) {
        if ignore || !intermediates.is_empty() {
            return;
        }

        match byte {
            b'7' => self.save_cursor(),
            b'8' => self.restore_cursor(),
            _ => {}
        }
    }
}

impl Screen {
    fn csi(&mut self, params: &Params, action: char) {
        let count = usize::from(default_param(params, 0, 1));

        match action {
            'A' => self.move_relative(0, negative(count)),
            'B' => self.move_relative(0, positive(count)),
            'C' => self.move_relative(positive(count), 0),
            'D' => self.move_relative(negative(count), 0),
            'E' => self.set_position(0, self.row().saturating_add(count)),
            'F' => self.set_position(0, self.row().saturating_sub(count)),
            'G' => self.set_position(count - 1, self.row()),
            'H' | 'f' => self.position(params),
            'J' => dispatch_erase(params, |mode| self.erase_display(mode)),
            'K' => dispatch_erase(params, |mode| self.erase_line(mode)),
            'd' => self.set_position(self.column(), count - 1),
            'm' => self.graphics(params),
            's' => self.save_cursor(),
            'u' => self.restore_cursor(),
            _ => {}
        }
    }

    fn position(&mut self, params: &Params) {
        let row = usize::from(default_param(params, 0, 1)) - 1;
        let column = usize::from(default_param(params, 1, 1)) - 1;

        self.set_position(column, row);
    }

    fn private_mode(&mut self, params: &Params, action: char) {
        let visible = match action {
            'h' => true,
            'l' => false,
            _ => return,
        };

        if params.iter().any(|param| param == [25]) {
            self.set_cursor_visible(visible);
        }
    }

    fn graphics(&mut self, params: &Params) {
        for param in params {
            let [code] = param else {
                continue;
            };

            match *code {
                0 => {
                    self.reset_foreground();
                    self.reset_background();
                }
                30..=37 => self.set_foreground(ANSI_COLORS[usize::from(*code - 30)]),
                39 => self.reset_foreground(),
                40..=47 => self.set_background(ANSI_COLORS[usize::from(*code - 40)]),
                49 => self.reset_background(),
                90..=97 => self.set_foreground(ANSI_COLORS[usize::from(*code - 82)]),
                100..=107 => self.set_background(ANSI_COLORS[usize::from(*code - 92)]),
                _ => {}
            }
        }
    }
}

fn raw_param(params: &Params, index: usize) -> u16 {
    params
        .iter()
        .nth(index)
        .and_then(|param| param.first())
        .copied()
        .unwrap_or(0)
}

fn default_param(params: &Params, index: usize, default: u16) -> u16 {
    match raw_param(params, index) {
        0 => default,
        value => value,
    }
}

fn dispatch_erase(params: &Params, erase: impl FnOnce(EraseMode)) {
    if let Ok(mode) = raw_param(params, 0).try_into() {
        erase(mode);
    }
}

fn positive(value: usize) -> isize {
    isize::try_from(value).unwrap_or(isize::MAX)
}

fn negative(value: usize) -> isize {
    -positive(value)
}
