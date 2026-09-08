/// Input settings owned by a line discipline.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(clippy::struct_excessive_bools)] // a bitmask-like settings bag reads best as plain fields
pub struct LineDisciplineSettings {
    pub echo: bool,
    pub canonical: bool,
    pub erase_character: u8,
    /// Whether control characters generate signals (the termios `ISIG` flag).
    pub isig: bool,
    /// The interrupt character (termios `VINTR`), conventionally Ctrl+C.
    pub intr_character: u8,
    /// Whether input carriage returns are mapped to newlines (termios `ICRNL`).
    pub icrnl: bool,
    /// Whether input newlines are mapped to carriage returns (termios `INLCR`).
    pub inlcr: bool,
    /// Whether input carriage returns are discarded (termios `IGNCR`).
    pub igncr: bool,
    /// Whether output post-processing is enabled at all (termios `OPOST`).
    pub opost: bool,
    /// Whether output newlines are mapped to CR+NL (termios `ONLCR`, effective under `OPOST`).
    pub onlcr: bool,
}

impl LineDisciplineSettings {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            echo: true,
            canonical: true,
            erase_character: b'\x08',
            isig: true,
            intr_character: b'\x03',
            icrnl: true,
            inlcr: false,
            igncr: false,
            opost: true,
            onlcr: true,
        }
    }
}

impl Default for LineDisciplineSettings {
    fn default() -> Self {
        Self::new()
    }
}
