use alloc::{boxed::Box, collections::VecDeque, sync::Arc};

use roxy_fd::{OpenFile, StatusFlags};
use roxy_poll::{PollListener, PollListeners, PollRegistration};
use roxy_thread::scheduler;
use roxy_utils::Lock;

use super::{connected::Connected, connection::Connection, side::Side, socket::Socket};

pub(super) enum BoundState {
    /// Bound but not yet listening; connection attempts must be refused.
    Bound,
    Listening {
        pending: VecDeque<Arc<OpenFile>>,
        backlog: usize,
    },
}

/// A bound server-side socket registered in the global bound-socket registry.
///
/// The registry holds this object only through a weak reference; the owning
/// [`Socket`](super::Socket) keeps the strong reference, so closing the last bound socket unbinds
/// its address.
pub(super) struct BoundSocket {
    pub(super) name: Arc<[u8]>,
    listeners: Arc<PollListeners>,
    state: Lock<BoundState>,
}

impl BoundSocket {
    pub(super) fn new(name: &[u8]) -> Self {
        Self {
            name: Arc::from(name),
            listeners: Arc::new(PollListeners::new()),
            state: Lock::new(BoundState::Bound),
        }
    }

    /// Transitions this socket from bound to listening with the given backlog capacity.
    ///
    /// # Errors
    ///
    /// Returns an error when the socket is already listening.
    pub(super) fn listen(&self, backlog: usize) -> Result<(), ()> {
        let mut state = self.state.lock();

        match &*state {
            BoundState::Bound => {
                *state = BoundState::Listening {
                    pending: VecDeque::new(),
                    backlog,
                };

                Ok(())
            }
            BoundState::Listening { .. } => Err(()),
        }
    }

    /// Removes one pending connection, blocking while the queue is empty.
    ///
    /// The listener state cannot leave the listening state, so blocking ends only through a
    /// connection arrival.
    ///
    /// # Errors
    ///
    /// Returns an error when this socket is not listening.
    pub(super) fn accept(self: &Arc<Self>) -> Result<Arc<OpenFile>, ()> {
        loop {
            // Holds the listener state lock across the readiness check, listener registration,
            // and block preparation. The lock disables preemption, so a concurrent enqueue cannot
            // notify between those steps.
            let (pending, registration) = {
                let mut state = self.state.lock();
                let BoundState::Listening { pending, .. } = &mut *state else {
                    return Err(());
                };

                if let Some(connection) = pending.pop_front() {
                    return Ok(connection);
                }

                let listener = PollListener::current_thread();
                let registration = self.listeners.register(listener.clone());
                let pending_block = scheduler::prepare_block_current_with_key(listener.wait_key());

                (pending_block, registration)
            };

            pending.perform();
            drop(registration);
        }
    }

    /// Registers a connection attempt against this listener.
    ///
    /// Creates the underlying connection, queues the server-side endpoint for `accept`, and
    /// returns the client-side connected state. Returns `None` when the socket is not listening
    /// or its backlog is full.
    pub(super) fn accept_connection_attempt(self: &Arc<Self>) -> Option<Connected> {
        let client = {
            let mut state = self.state.lock();
            let BoundState::Listening { pending, backlog } = &mut *state else {
                return None;
            };

            if pending.len() >= *backlog {
                return None;
            }

            let connection = Arc::new(Connection::new());
            let endpoint = OpenFile::new(Box::new(Socket::connected(
                connection.clone(),
                Side::Second,
                Some(self.name.clone()),
            )));
            endpoint.set_status_flags(StatusFlags::READ_WRITE);
            pending.push_back(endpoint);

            Some(Connected::new(connection, Side::First))
        };

        // State mutations release the state lock before notifying listeners so the connection and
        // scheduler locks are never nested.
        self.listeners.notify();
        client
    }

    /// Registers a poll listener for connection arrivals on this socket.
    pub(super) fn register_poll_listener(&self, listener: Arc<PollListener>) -> PollRegistration {
        self.listeners.register(listener)
    }

    /// Reports whether the accept queue holds at least one pending connection.
    pub(super) fn has_pending_connection(&self) -> bool {
        let state = self.state.lock();

        matches!(
            &*state,
            BoundState::Listening { pending, .. } if !pending.is_empty()
        )
    }
}
