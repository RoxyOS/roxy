#![no_std]

extern crate alloc;

mod pair;
mod registry;

use alloc::sync::Arc;

use spin::Once;

pub use pair::{PtyMaster, PtyPair, PtySlave};
pub use registry::PtyRegistry;

pub(crate) static REGISTRY: Once<Arc<PtyRegistry>> = Once::new();

/// The process-wide pty registry, which is also the `/dev/ptmx` factory and the `/dev/pts/N`
/// dynamic device resolver.
///
/// kernel-main registers this object both under `ptmx` and as the dynamic resolver of `roxy-devfs`,
/// so opening `/dev/ptmx` allocates a fresh pair and opening `/dev/pts/N` reaches its slave.
#[must_use]
pub fn registry() -> Arc<PtyRegistry> {
    REGISTRY.call_once(|| Arc::new(PtyRegistry::new())).clone()
}
