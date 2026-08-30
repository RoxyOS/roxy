use alloc::sync::Arc;

use crate::{OutputError, TerminalOutput};

/// A terminal endpoint that mirrors writes to a primary display and a secondary endpoint.
///
/// The primary endpoint's window size and write result are authoritative: `write` reports the
/// primary endpoint's progress and the secondary endpoint receives exactly the bytes the primary
/// accepted, so callers that retry partial writes (e.g. `kernel_terminal::print`) never feed the
/// mirror a different slice of the output than the primary.
///
/// Writes to the mirror are best-effort: an endpoint failure on the secondary side does not fail
/// the primary display, so a broken mirror can never suppress ordinary kernel output.
pub struct TeeOutput {
    primary: Arc<dyn TerminalOutput>,
    mirror: Arc<dyn TerminalOutput>,
}

impl TeeOutput {
    /// Returns a terminal that mirrors `mirror` onto every write the `primary` accepts.
    #[must_use]
    pub fn new(primary: Arc<dyn TerminalOutput>, mirror: Arc<dyn TerminalOutput>) -> Self {
        Self { primary, mirror }
    }
}

impl TerminalOutput for TeeOutput {
    fn write(&self, input: &[u8]) -> Result<usize, OutputError> {
        let written = self.primary.write(input)?;
        let _ = self.mirror.write(&input[..written]);
        Ok(written)
    }

    fn window_size(&self) -> roxy_tty_types::WindowSize {
        self.primary.window_size()
    }
}
