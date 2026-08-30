use alloc::{sync::Arc, vec::Vec};

use crate::OpenFile;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SocketError {
    AddressInUse,
    AlreadyConnected,
    ConnectionRefused,
    InvalidState,
    Io,
}

/// The direction of a [`shutdown`](SocketOps::shutdown) call.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShutdownHow {
    /// Disable further receives on this socket.
    Rd,
    /// Disable further sends on this socket.
    Wr,
    /// Disable both receives and sends on this socket.
    RdWr,
}

impl ShutdownHow {
    /// Decodes the raw `how` argument of the `shutdown(2)` ABI (`SHUT_RD`=0, `SHUT_WR`=1,
    /// `SHUT_RDWR`=2).
    #[must_use]
    pub const fn from_raw(raw: u64) -> Option<Self> {
        match raw {
            0 => Some(Self::Rd),
            1 => Some(Self::Wr),
            2 => Some(Self::RdWr),
            _ => None,
        }
    }
}

/// Socket-specific operations exposed by file objects that can act as sockets.
///
/// Path arguments are normalized absolute paths; the caller resolves relative paths against the
/// working directory before dispatching. Implementations may block while waiting for connection
/// state, matching the blocking behavior of the `File` read and write operations.
pub trait SocketOps: Send {
    /// Registers this socket under `name` in the kernel's bound-socket registry.
    ///
    /// # Errors
    ///
    /// Returns an error when another live socket is already bound to `name` or the socket is not
    /// in an unbound state.
    fn bind(&mut self, name: &[u8]) -> Result<(), SocketError>;

    /// Enables connection acceptance for this socket with the given backlog capacity.
    ///
    /// # Errors
    ///
    /// Returns an error when the socket is not bound or is already listening.
    fn listen(&mut self, backlog: u32) -> Result<(), SocketError>;

    /// Removes one pending connection from the accept queue and returns it as an open file.
    ///
    /// Blocks while the queue is empty, like a blocking read.
    ///
    /// # Errors
    ///
    /// Returns an error when the socket is not listening.
    fn accept(&mut self) -> Result<Arc<OpenFile>, SocketError>;

    /// Connects this socket to the server bound at `name`.
    ///
    /// # Errors
    ///
    /// Returns an error when no live socket is listening at `name`, its accept backlog is full,
    /// or this socket is already connected.
    fn connect(&mut self, name: &[u8]) -> Result<(), SocketError>;

    /// Disables one or both directions of an established connection.
    ///
    /// # Errors
    ///
    /// Returns an error when the socket is not connected.
    fn shutdown(&mut self, how: ShutdownHow) -> Result<(), SocketError>;

    /// Reads a socket option into `buffer`, returning the number of bytes written.
    ///
    /// Only a minimal `SOL_SOCKET` subset is supported (`SO_TYPE`, `SO_ERROR`); unsupported
    /// options return an error.
    ///
    /// # Errors
    ///
    /// Returns an error when the option is unsupported.
    fn get_sockopt(
        &mut self,
        layer: u32,
        number: u32,
        buffer: &mut [u8],
    ) -> Result<usize, SocketError>;

    /// Returns the local address bound to this socket, or `None` when the socket is unnamed.
    ///
    /// # Errors
    ///
    /// Returns an error when the socket is not bound or connected.
    fn sockname(&mut self) -> Result<Option<Vec<u8>>, SocketError>;

    /// Returns the address of the connected peer, or `None` when the peer is unnamed.
    ///
    /// # Errors
    ///
    /// Returns an error when the socket is not connected.
    fn peername(&mut self) -> Result<Option<Vec<u8>>, SocketError>;
}
