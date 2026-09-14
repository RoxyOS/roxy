#![no_std]

extern crate alloc;

mod pair;

use alloc::{boxed::Box, sync::Arc};

use roxy_fd::OpenFile;

use pair::{PtyMaster, PtyPair, PtySlave};

/// Allocates a pseudo-terminal pair and returns its master and slave as open file descriptions.
///
/// The pair has no device-filesystem name: the slave is reachable only through the returned
/// descriptor, so a caller that needs it in a child process passes the descriptor across `fork`
/// rather than reopening a path. The master carries no terminal semantics; the slave owns the line
/// discipline and termios, and becomes a session's controlling terminal through `TIOCSCTTY`.
#[must_use]
pub fn open_pair() -> (Arc<OpenFile>, Arc<OpenFile>) {
    let pair = PtyPair::new();

    (
        OpenFile::new(Box::new(PtyMaster::new(pair.clone()))),
        OpenFile::new(Box::new(PtySlave::new(pair))),
    )
}
