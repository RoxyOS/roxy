/// Input settings owned by a line discipline.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LineDisciplineSettings {
    pub echo: bool,
    pub canonical: bool,
    pub erase_character: u8,
}

impl LineDisciplineSettings {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            echo: true,
            canonical: true,
            erase_character: b'\x08',
        }
    }
}

impl Default for LineDisciplineSettings {
    fn default() -> Self {
        Self::new()
    }
}
