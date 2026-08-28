use alloc::sync::Arc;

use roxy_fd::{
    File, FileError, FileMetadata, FileType, OpenFile, PollEvents, SeekError, SeekFrom,
    SocketError, SocketOps,
};
use roxy_poll::{PollListener, PollRegistration};
use roxy_utils::Lock;

use super::{
    bound::BoundSocket, connected::Connected, connection::Connection, registry, side::Side,
};

pub(super) enum SocketInner {
    /// Created but not bound or connected.
    Initial,
    /// Registered in the bound-socket registry; accepts connections once listening.
    Bound(Arc<BoundSocket>),
    /// Connected as one side of an established connection.
    Connected(Connected),
}

impl Drop for SocketInner {
    fn drop(&mut self) {
        // Connected sockets unbind their channel side through `Connected::drop`; only the bound
        // state owns a registry entry that must be removed explicitly.
        if let SocketInner::Bound(bound) = self {
            registry::remove(bound);
        }
    }
}

/// One Unix stream socket in any lifecycle state: created, bound, listening, or connected.
pub struct Socket {
    inner: Lock<SocketInner>,
}

impl Socket {
    pub(super) const fn new() -> Self {
        Self {
            inner: Lock::new(SocketInner::Initial),
        }
    }

    /// Creates a socket already connected as `side` of `connection`.
    pub(super) fn connected(connection: Arc<Connection>, side: Side) -> Self {
        Self {
            inner: Lock::new(SocketInner::Connected(Connected::new(connection, side))),
        }
    }
}

impl File for Socket {
    fn poll(&mut self) -> Result<PollEvents, FileError> {
        let inner = self.inner.lock();

        match &*inner {
            SocketInner::Initial => Ok(PollEvents::default()),
            SocketInner::Bound(bound) => Ok(PollEvents {
                readable: bound.has_pending_connection(),
                ..PollEvents::default()
            }),
            SocketInner::Connected(connected) => Ok(connected.poll()),
        }
    }

    fn register_poll_listener(&mut self, listener: Arc<PollListener>) -> PollRegistration {
        let inner = self.inner.lock();

        match &*inner {
            SocketInner::Initial => PollRegistration::inactive(),
            SocketInner::Bound(bound) => bound.register_poll_listener(listener),
            SocketInner::Connected(connected) => connected.register_poll_listener(listener),
        }
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
        let mut inner = self.inner.lock();

        match &mut *inner {
            SocketInner::Connected(connected) => connected.read(output),
            SocketInner::Initial | SocketInner::Bound(_) => Err(FileError::NotConnected),
        }
    }

    fn write(&mut self, _position: &mut u64, input: &[u8]) -> Result<usize, FileError> {
        let mut inner = self.inner.lock();

        match &mut *inner {
            SocketInner::Connected(connected) => connected.write(input),
            SocketInner::Initial | SocketInner::Bound(_) => Err(FileError::NotConnected),
        }
    }

    fn seek(&mut self, _current: u64, _position: SeekFrom) -> Result<u64, SeekError> {
        Err(SeekError::NotSeekable)
    }

    fn as_socket(&mut self) -> Option<&mut dyn SocketOps> {
        Some(self)
    }
}

impl SocketOps for Socket {
    fn bind(&mut self, name: &[u8]) -> Result<(), SocketError> {
        let mut inner = self.inner.lock();

        if !matches!(&*inner, SocketInner::Initial) {
            return Err(SocketError::InvalidState);
        }

        let bound = Arc::new(BoundSocket::new(name));

        if !registry::insert(&bound) {
            return Err(SocketError::AddressInUse);
        }

        *inner = SocketInner::Bound(bound);

        Ok(())
    }

    fn listen(&mut self, backlog: u32) -> Result<(), SocketError> {
        let bound = {
            let inner = self.inner.lock();
            match &*inner {
                SocketInner::Bound(bound) => bound.clone(),
                SocketInner::Initial | SocketInner::Connected(_) => {
                    return Err(SocketError::InvalidState);
                }
            }
        };

        bound
            .listen(usize::try_from(backlog).unwrap_or(usize::MAX))
            .map_err(|()| SocketError::InvalidState)
    }

    fn accept(&mut self) -> Result<Arc<OpenFile>, SocketError> {
        let bound = {
            let inner = self.inner.lock();
            match &*inner {
                SocketInner::Bound(bound) => bound.clone(),
                SocketInner::Initial | SocketInner::Connected(_) => {
                    return Err(SocketError::InvalidState);
                }
            }
        };

        bound.accept().map_err(|()| SocketError::InvalidState)
    }

    fn connect(&mut self, name: &[u8]) -> Result<(), SocketError> {
        let mut inner = self.inner.lock();

        match &*inner {
            SocketInner::Initial => {}
            SocketInner::Bound(_) => return Err(SocketError::InvalidState),
            SocketInner::Connected(_) => return Err(SocketError::AlreadyConnected),
        }

        let listener = registry::lookup(name).ok_or(SocketError::ConnectionRefused)?;
        let client = listener
            .accept_connection_attempt()
            .ok_or(SocketError::ConnectionRefused)?;

        *inner = SocketInner::Connected(client);

        Ok(())
    }
}

#[cfg(feature = "kernel-test")]
mod tests {

    use roxy_fd::{File, FileError, SocketError, SocketOps};
    use roxy_test::kernel_test;

    use super::Socket;

    fn socket() -> Socket {
        Socket::new()
    }

    kernel_test!(
        "roxy-unix-socket::socket",
        connects_through_bound_listener,
        {
            let mut server = socket();
            let mut client = socket();

            assert_eq!(server.bind(b"/tmp/roxy-test.sock"), Ok(()));
            assert_eq!(server.listen(4), Ok(()));
            assert_eq!(client.connect(b"/tmp/roxy-test.sock"), Ok(()));

            let connection = server.accept().unwrap();
            let mut output = [0; 4];

            assert_eq!(client.write(&mut 0, b"ping"), Ok(4));
            assert_eq!(connection.read(&mut output), Ok(4));
            assert_eq!(&output, b"ping");
        }
    );

    kernel_test!(
        "roxy-unix-socket::socket",
        refuses_second_bind_on_same_address,
        {
            let mut first = socket();
            let mut second = socket();

            assert_eq!(first.bind(b"/tmp/roxy-test.sock"), Ok(()));
            assert_eq!(
                second.bind(b"/tmp/roxy-test.sock"),
                Err(SocketError::AddressInUse)
            );
        }
    );

    kernel_test!("roxy-unix-socket::socket", unbinds_address_on_drop, {
        let mut first = socket();
        let mut second = socket();

        assert_eq!(first.bind(b"/tmp/roxy-test.sock"), Ok(()));
        drop(first);

        assert_eq!(second.bind(b"/tmp/roxy-test.sock"), Ok(()));
    });

    kernel_test!(
        "roxy-unix-socket::socket",
        refuses_connect_without_listener,
        {
            let mut client = socket();

            assert_eq!(
                client.connect(b"/tmp/roxy-test.sock"),
                Err(SocketError::ConnectionRefused)
            );
        }
    );

    kernel_test!(
        "roxy-unix-socket::socket",
        refuses_connect_on_bound_but_not_listening_socket,
        {
            let mut bound = socket();
            let mut client = socket();

            assert_eq!(bound.bind(b"/tmp/roxy-test.sock"), Ok(()));
            assert_eq!(
                client.connect(b"/tmp/roxy-test.sock"),
                Err(SocketError::ConnectionRefused)
            );
        }
    );

    kernel_test!(
        "roxy-unix-socket::socket",
        refuses_connect_when_backlog_is_full,
        {
            let mut server = socket();
            let mut first = socket();
            let mut second = socket();

            assert_eq!(server.bind(b"/tmp/roxy-test.sock"), Ok(()));
            assert_eq!(server.listen(1), Ok(()));
            assert_eq!(first.connect(b"/tmp/roxy-test.sock"), Ok(()));
            assert_eq!(
                second.connect(b"/tmp/roxy-test.sock"),
                Err(SocketError::ConnectionRefused)
            );
        }
    );

    kernel_test!(
        "roxy-unix-socket::socket",
        rejects_operations_in_wrong_state,
        {
            let mut unbound = socket();
            let mut bound = socket();
            let mut client = socket();

            assert_eq!(unbound.listen(1), Err(SocketError::InvalidState));
            assert!(matches!(unbound.accept(), Err(SocketError::InvalidState)));
            assert_eq!(unbound.bind(b"/tmp/roxy-test.sock"), Ok(()));

            assert_eq!(
                bound.bind(b"/tmp/other-test.sock"),
                Err(SocketError::InvalidState)
            );
            assert_eq!(bound.listen(1), Ok(()));
            assert_eq!(bound.listen(1), Err(SocketError::InvalidState));
            assert_eq!(
                bound.bind(b"/tmp/roxy-test.sock"),
                Err(SocketError::InvalidState)
            );

            assert_eq!(client.connect(b"/tmp/other-test.sock"), Ok(()));
            assert_eq!(
                client.connect(b"/tmp/other-test.sock"),
                Err(SocketError::AlreadyConnected)
            );
            assert_eq!(
                client.bind(b"/tmp/roxy-test.sock"),
                Err(SocketError::InvalidState)
            );
        }
    );

    kernel_test!("roxy-unix-socket::socket", rejects_io_before_connection, {
        let mut unbound = socket();

        assert_eq!(
            unbound.read(&mut 0, &mut [0; 1]),
            Err(FileError::NotConnected)
        );
        assert_eq!(unbound.write(&mut 0, b"x"), Err(FileError::NotConnected));
    });

    kernel_test!(
        "roxy-unix-socket::socket",
        reports_pending_connections_as_readable,
        {
            let mut server = socket();
            let mut client = socket();

            assert_eq!(server.bind(b"/tmp/roxy-test.sock"), Ok(()));
            assert_eq!(server.listen(4), Ok(()));

            let idle = server.poll().unwrap();
            assert!(!idle.readable);

            assert_eq!(client.connect(b"/tmp/roxy-test.sock"), Ok(()));

            let pending = server.poll().unwrap();
            assert!(pending.readable);

            assert!(server.accept().is_ok());

            let drained = server.poll().unwrap();
            assert!(!drained.readable);
        }
    );
}
