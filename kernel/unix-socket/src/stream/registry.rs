use alloc::{
    sync::{Arc, Weak},
    vec::Vec,
};
use roxy_utils::Lock;

use super::bound::BoundSocket;

/// Registry of live bound sockets, keyed by normalized address.
///
/// Entries hold weak references so a socket that was closed without cleanup disappears from the
/// registry; lookups and inserts prune dead entries. Each address is held by at most one live
/// socket.
static BOUND_SOCKETS: Lock<Vec<Weak<BoundSocket>>> = Lock::new(Vec::new());

/// Registers `socket` and prunes dead entries.
///
/// Returns `false` when another live socket is already bound to the same address.
pub(super) fn insert(socket: &Arc<BoundSocket>) -> bool {
    let mut sockets = BOUND_SOCKETS.lock();
    sockets.retain(|candidate| candidate.upgrade().is_some());

    let taken = sockets.iter().any(|candidate| {
        candidate
            .upgrade()
            .is_some_and(|live| live.name == socket.name)
    });

    if taken {
        return false;
    }

    sockets.push(Arc::downgrade(socket));

    true
}

/// Returns the live socket bound at `name`, pruning dead entries.
pub(super) fn lookup(name: &[u8]) -> Option<Arc<BoundSocket>> {
    let mut sockets = BOUND_SOCKETS.lock();
    sockets.retain(|candidate| candidate.upgrade().is_some());

    sockets.iter().find_map(|candidate| {
        candidate
            .upgrade()
            .filter(|live| live.name.as_ref() == name)
    })
}

/// Removes the registry entry that points at `socket`, pruning dead entries.
pub(super) fn remove(socket: &Arc<BoundSocket>) {
    BOUND_SOCKETS.lock().retain(|candidate| {
        candidate
            .upgrade()
            .is_none_or(|live| !Arc::ptr_eq(&live, socket))
    });
}
