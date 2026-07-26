#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RgbColor {
    pub(crate) red: u8,
    pub(crate) green: u8,
    pub(crate) blue: u8,
}

impl RgbColor {
    pub(crate) const BLACK: Self = Self::new(0, 0, 0);
    pub(crate) const WHITE: Self = Self::new(0xff, 0xff, 0xff);

    pub(crate) const fn new(red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue }
    }
}
