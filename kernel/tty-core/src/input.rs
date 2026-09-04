use alloc::vec::Vec;

/// Provides the byte input stream a terminal feeds through its line discipline.
///
/// This is the boundary that lets a console terminal (keyboard events decoded into bytes) and a
/// pty slave (bytes written by the pty master) share one terminal implementation. Only the input
/// side differs; everything downstream of the returned bytes is terminal-common.
pub trait TerminalInputSource: Send + Sync {
    /// Pops and returns the next input payload for the line discipline, or `None` when nothing is
    /// pending. Called from the blocking read path, which may take non-try locks. Implementations
    /// may decode here (for example mapping a raw key event to a character or escape sequence) and
    /// should skip inputs that produce no output.
    fn next_input_bytes(&self) -> Option<Vec<u8>>;

    /// IRQ/callback-safe peek: decodes the next pending input into its bytes **without consuming
    /// it**. Must use `try_lock` internally and never mutate the pending set.
    ///
    /// This lets the terminal's interrupt-time fast path acquire the line-discipline lock after a
    /// successful decode (mirroring the "pop only once both locks are held" ordering) without ever
    /// losing an input: the caller only calls [`Self::consume_peeked`] once it can process it.
    fn try_peek_bytes(&self) -> Option<Vec<u8>>;

    /// Consumes the input peeked by [`Self::try_peek_bytes`]. Must be IRQ/callback-safe.
    fn consume_peeked(&self);

    /// Discards all pending input (used by `TCSAFLUSH`). Must be IRQ/callback-safe.
    fn discard_pending_input(&self);
}
