use alloc::sync::Arc;

use crate::OpenFile;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SocketError {
    AddressInUse,
    AlreadyConnected,
    ConnectionRefused,
    InvalidState,
    Io,
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
}
