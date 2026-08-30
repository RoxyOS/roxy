use alloc::sync::Arc;

use roxy_fd::{FileError, PollEvents, ShutdownHow};
use roxy_poll::{PollListener, PollRegistration};
use roxy_thread::scheduler;

use super::{
    buffer::CAPACITY,
    connection::{Connection, State},
    side::Side,
};

/// The connected payload of a [`Socket`](super::Socket): one side of an established connection.
///
/// Owns one direction of the underlying channel. The connection stays alive while either side
/// exists, and closing this side through `Drop` wakes the peer.
pub(super) struct Connected {
    connection: Arc<Connection>,
    side: Side,
}

impl Connected {
    pub(super) fn new(connection: Arc<Connection>, side: Side) -> Self {
        Self { connection, side }
    }

    pub(super) fn read(
        &mut self,
        output: &mut [u8],
        nonblocking: bool,
    ) -> Result<usize, FileError> {
        if output.is_empty() {
            return Ok(0);
        }

        loop {
            let (pending, registration) = {
                let mut states = self.connection.states.lock();
                let index = self.side.index();
                let peer = self.side.other().index();

                let state = &mut states[index];

                if !state.received_data.is_empty() {
                    // Read first so the mutable borrow ends before the listener sets are cloned.
                    let count = state.received_data.read_to(output);
                    let (self_listener, peer_listener) = (
                        states[index].listeners.clone(),
                        states[peer].listeners.clone(),
                    );

                    drop(states);
                    self_listener.notify();
                    peer_listener.notify();

                    return Ok(count);
                }

                // This side has shut down receiving; report EOF once buffered data is drained.
                if !state.open_read {
                    return Ok(0);
                }

                // The peer will no longer send (half-closed or fully closed); report EOF.
                if !states[peer].open_write {
                    return Ok(0);
                }

                if nonblocking {
                    return Err(FileError::WouldBlock);
                }

                self.prepare_wait(&states)
            };

            pending.perform();
            drop(registration);
        }
    }

    pub(super) fn write(&mut self, input: &[u8], nonblocking: bool) -> Result<usize, FileError> {
        if input.is_empty() {
            return Ok(0);
        }

        loop {
            let (pending, registration) = {
                let mut states = self.connection.states.lock();
                let index = self.side.index();
                let peer = self.side.other().index();

                if !states[index].open_write || !states[peer].open_read {
                    return Err(FileError::BrokenPipe);
                }

                let available = CAPACITY - states[peer].received_data.len();
                if available > 0 {
                    // Write first so the mutable borrow ends before the listener sets are cloned.
                    let count = states[peer].received_data.write_from(input);
                    let (self_listener, peer_listener) = (
                        states[index].listeners.clone(),
                        states[peer].listeners.clone(),
                    );

                    drop(states);
                    self_listener.notify();
                    peer_listener.notify();

                    return Ok(count);
                }

                if nonblocking {
                    return Err(FileError::WouldBlock);
                }

                self.prepare_wait(&states)
            };

            pending.perform();
            drop(registration);
        }
    }

    /// Disables one or both directions on this side of the connection and wakes any peer that
    /// is blocked on the resulting readiness change.
    pub(super) fn shutdown(&mut self, how: ShutdownHow) {
        let (self_listener, peer_listener) = {
            let mut states = self.connection.states.lock();
            let index = self.side.index();
            let peer = self.side.other().index();

            let state = &mut states[index];
            match how {
                ShutdownHow::Rd => state.open_read = false,
                ShutdownHow::Wr => state.open_write = false,
                ShutdownHow::RdWr => {
                    state.open_read = false;
                    state.open_write = false;
                }
            }

            (
                states[index].listeners.clone(),
                states[peer].listeners.clone(),
            )
        };

        self_listener.notify();
        peer_listener.notify();
    }

    pub(super) fn poll(&self) -> PollEvents {
        let states = self.connection.states.lock();
        let index = self.side.index();
        let peer = self.side.other().index();

        let state = &states[index];
        let peer_state = &states[peer];

        PollEvents {
            readable: !state.open_read || !state.received_data.is_empty() || !peer_state.open_write,
            writable: state.open_write
                && peer_state.open_read
                && peer_state.received_data.len() < CAPACITY,
            hangup: !state.open_read || !peer_state.open_write,
            ..PollEvents::default()
        }
    }

    pub(super) fn register_poll_listener(&self, listener: Arc<PollListener>) -> PollRegistration {
        let states = self.connection.states.lock();

        states[self.side.index()].listeners.register(listener)
    }

    fn prepare_wait(&self, states: &[State; 2]) -> (scheduler::PendingBlock, PollRegistration) {
        // Registers a listener for this side's state to be woken on readiness changes, then
        // prepares the current thread to block on that listener's wait key.
        let listener = PollListener::current_thread();
        let registration = states[self.side.index()]
            .listeners
            .register(listener.clone());

        let pending = scheduler::prepare_block_current_with_key(listener.wait_key());

        (pending, registration)
    }
}

impl Drop for Connected {
    fn drop(&mut self) {
        let peer_listeners = {
            let mut states = self.connection.states.lock();
            let index = self.side.index();
            let peer = self.side.other().index();

            states[index].open_read = false;
            states[index].open_write = false;
            states[index].received_data.clear();
            states[peer].listeners.clone()
        };

        peer_listeners.notify();
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_fd::{FileError, FileType, SeekError, SeekFrom};
    use roxy_test::kernel_test;

    use super::super::buffer::CAPACITY;
    use super::super::pair;

    kernel_test!("roxy-unix-socket::connected", transfers_bidirectionally, {
        let (first, second) = pair();
        let mut output = [0; 4];

        assert_eq!(first.write(b"ping"), Ok(4));
        assert_eq!(second.read(&mut output), Ok(4));
        assert_eq!(&output, b"ping");
        assert_eq!(second.write(b"pong"), Ok(4));
        assert_eq!(first.read(&mut output), Ok(4));
        assert_eq!(&output, b"pong");
    });

    kernel_test!("roxy-unix-socket::connected", reports_socket_metadata, {
        let (first, _) = pair();

        assert_eq!(first.metadata().unwrap().file_type, FileType::Socket);
        assert_eq!(first.seek(SeekFrom::Start(0)), Err(SeekError::NotSeekable));
    });

    kernel_test!("roxy-unix-socket::connected", reports_readiness, {
        let (first, second) = pair();

        let initial = second.poll().unwrap();
        assert!(!initial.readable);
        assert!(initial.writable);
        assert!(!initial.hangup);

        assert_eq!(first.write(b"x"), Ok(1));
        let received = second.poll().unwrap();
        assert!(received.readable);
        assert!(received.writable);
        assert!(!received.hangup);
    });

    kernel_test!("roxy-unix-socket::connected", respects_buffer_capacity, {
        let (first, _) = pair();
        let input = alloc::vec![7; CAPACITY + 1];

        assert_eq!(first.write(&input), Ok(CAPACITY));
    });

    kernel_test!("roxy-unix-socket::connected", reports_peer_close, {
        let (first, second) = pair();
        drop(second);

        assert_eq!(first.read(&mut [0; 1]), Ok(0));
        assert_eq!(first.write(b"x"), Err(FileError::BrokenPipe));
        let events = first.poll().unwrap();
        assert!(events.readable);
        assert!(events.hangup);
        assert!(!events.writable);
    });
}
