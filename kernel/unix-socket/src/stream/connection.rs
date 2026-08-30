use alloc::sync::Arc;

use roxy_poll::PollListeners;
use roxy_utils::Lock;

use super::buffer::Buffer;

pub(super) struct Connection {
    pub(super) states: Lock<[State; 2]>,
}

pub(super) struct State {
    pub(super) received_data: Buffer,
    /// This side can still receive data from the peer.
    pub(super) open_read: bool,
    /// This side can still send data to the peer.
    pub(super) open_write: bool,

    /// Listeners listening for changes for this endpoint.
    pub(super) listeners: Arc<PollListeners>,
}

impl Connection {
    pub(super) fn new() -> Self {
        Self {
            states: Lock::new([State::new(), State::new()]),
        }
    }
}

impl State {
    fn new() -> Self {
        Self {
            received_data: Buffer::new(),
            open_read: true,
            open_write: true,
            listeners: Arc::new(PollListeners::new()),
        }
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_test::kernel_test;

    use super::Connection;

    kernel_test!(
        "roxy-unix-socket::connection",
        initializes_two_open_states,
        {
            let connection = Connection::new();
            let states = connection.states.lock();

            assert!(states.iter().all(|state| state.open_read));
            assert!(states.iter().all(|state| state.open_write));
            assert!(states.iter().all(|state| state.received_data.is_empty()));
        }
    );

    kernel_test!("roxy-unix-socket::connection", exposes_capacity_limit, {
        assert_eq!(super::super::buffer::CAPACITY, 64 * 1024);
    });
}
