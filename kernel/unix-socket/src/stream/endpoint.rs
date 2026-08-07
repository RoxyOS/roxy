use alloc::sync::Arc;

use roxy_fd::{File, FileError, FileMetadata, FileType, PollEvents, SeekError, SeekFrom};
use roxy_poll::{PollListener, PollRegistration};
use roxy_thread::scheduler;

use super::{
    buffer::CAPACITY,
    connection::{Connection, State},
    side::Side,
};

pub(super) struct Endpoint {
    connection: Arc<Connection>,
    side: Side,
}

impl Endpoint {
    pub(super) const fn new(connection: Arc<Connection>, side: Side) -> Self {
        Self { connection, side }
    }

    fn prepare_wait(&self, states: &[State; 2]) -> (scheduler::PendingBlock, PollRegistration) {
        // Registers a listener for this endpoint state to be woken on read/write to it.
        let listener = PollListener::current_thread();
        let registration = states[self.side.index()]
            .listeners
            .register(listener.clone());

        let pending = scheduler::prepare_block_current_with_key(listener.wait_key());

        (pending, registration)
    }
}

impl Drop for Endpoint {
    fn drop(&mut self) {
        let peer_listeners = {
            let mut states = self.connection.states.lock();
            let index = self.side.index();
            let peer = self.side.other().index();

            states[index].open = false;
            states[index].received_data.clear();
            states[peer].listeners.clone()
        };

        peer_listeners.notify();
    }
}

impl File for Endpoint {
    fn poll(&mut self) -> Result<PollEvents, FileError> {
        let states = self.connection.states.lock();
        let index = self.side.index();
        let peer = self.side.other().index();

        let state = &states[index];
        let peer_state = &states[peer];

        Ok(PollEvents {
            readable: !state.received_data.is_empty() || !peer_state.open,
            writable: peer_state.open && peer_state.received_data.len() < CAPACITY,
            hangup: !peer_state.open,
            ..PollEvents::default()
        })
    }

    fn register_poll_listener(&mut self, listener: Arc<PollListener>) -> PollRegistration {
        let states = self.connection.states.lock();
        states[self.side.index()].listeners.register(listener)
    }

    fn is_terminal(&self) -> bool {
        false
    }

    fn metadata(&self) -> Result<FileMetadata, FileError> {
        Ok(FileMetadata {
            file_id: 0,
            file_type: FileType::Socket,
            permissions: 0o600,
            size: 0,
            hard_links: 1,
        })
    }

    fn read(&mut self, _position: &mut u64, output: &mut [u8]) -> Result<usize, FileError> {
        if output.is_empty() {
            return Ok(0);
        }

        loop {
            let (pending, registration) = {
                let mut states = self.connection.states.lock();

                let state_index = self.side.index();
                let peer_index = self.side.other().index();

                let peer_is_open = states[peer_index].open;

                let self_state = &mut states[state_index];

                if !self_state.received_data.is_empty() {
                    // Read buffer to `output`
                    let count = self_state.received_data.read_to(output);

                    // Notify poll listeners
                    let (self_listener, peer_listener) = (
                        states[self.side.index()].listeners.clone(),
                        states[self.side.other().index()].listeners.clone(),
                    );

                    drop(states);
                    self_listener.notify();
                    peer_listener.notify();

                    return Ok(count);
                }

                if !peer_is_open {
                    return Ok(0);
                }

                // No data, wait
                self.prepare_wait(&states)
            };

            pending.perform();
            drop(registration);
        }
    }

    fn write(&mut self, _position: &mut u64, input: &[u8]) -> Result<usize, FileError> {
        if input.is_empty() {
            return Ok(0);
        }

        loop {
            let (pending, registration) = {
                let mut states = self.connection.states.lock();
                let peer_index = self.side.other().index();
                let peer_state = &mut states[peer_index];

                if !peer_state.open {
                    return Err(FileError::BrokenPipe);
                }

                let available = CAPACITY - peer_state.received_data.len();
                if available > 0 {
                    // Write to peer buffer
                    let count = peer_state.received_data.write_from(input);

                    // Notify poll listeners
                    let (self_listener, peer_listener) = (
                        states[self.side.index()].listeners.clone(),
                        states[self.side.other().index()].listeners.clone(),
                    );

                    drop(states);
                    self_listener.notify();
                    peer_listener.notify();

                    return Ok(count);
                }

                self.prepare_wait(&states)
            };

            pending.perform();
            drop(registration);
        }
    }

    fn seek(&mut self, _current: u64, _position: SeekFrom) -> Result<u64, SeekError> {
        Err(SeekError::NotSeekable)
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_fd::{FileType, SeekError, SeekFrom};
    use roxy_test::kernel_test;

    use super::super::buffer::CAPACITY;
    use super::super::pair;

    kernel_test!("roxy-unix-socket::endpoint", transfers_bidirectionally, {
        let (first, second) = pair();
        let mut output = [0; 4];

        assert_eq!(first.write(b"ping"), Ok(4));
        assert_eq!(second.read(&mut output), Ok(4));
        assert_eq!(&output, b"ping");
        assert_eq!(second.write(b"pong"), Ok(4));
        assert_eq!(first.read(&mut output), Ok(4));
        assert_eq!(&output, b"pong");
    });

    kernel_test!("roxy-unix-socket::endpoint", reports_socket_metadata, {
        let (first, _) = pair();

        assert_eq!(first.metadata().unwrap().file_type, FileType::Socket);
        assert_eq!(first.seek(SeekFrom::Start(0)), Err(SeekError::NotSeekable));
    });

    kernel_test!("roxy-unix-socket::endpoint", reports_readiness, {
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

    kernel_test!("roxy-unix-socket::endpoint", respects_buffer_capacity, {
        let (first, _) = pair();
        let input = alloc::vec![7; CAPACITY + 1];

        assert_eq!(first.write(&input), Ok(CAPACITY));
    });

    kernel_test!("roxy-unix-socket::endpoint", reports_peer_close, {
        let (first, second) = pair();
        drop(second);

        assert_eq!(first.read(&mut [0; 1]), Ok(0));
        assert_eq!(first.write(b"x"), Err(roxy_fd::FileError::BrokenPipe));
        let events = first.poll().unwrap();
        assert!(events.readable);
        assert!(events.hangup);
        assert!(!events.writable);
    });
}
