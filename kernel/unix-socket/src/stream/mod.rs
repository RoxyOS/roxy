use alloc::{boxed::Box, sync::Arc};

use roxy_fd::OpenFile;

mod bound;
mod buffer;
mod connected;
mod connection;
mod registry;
mod side;
mod socket;

use connection::Connection;
use side::Side;

pub use socket::Socket;

/// Creates two connected, bidirectional Unix stream sockets.
#[must_use]
pub fn pair() -> (Arc<OpenFile>, Arc<OpenFile>) {
    let connection = Arc::new(Connection::new());

    (
        OpenFile::new(Box::new(Socket::connected(connection.clone(), Side::First))),
        OpenFile::new(Box::new(Socket::connected(connection, Side::Second))),
    )
}

/// Creates one unconnected Unix stream socket.
#[must_use]
pub fn socket() -> Arc<OpenFile> {
    OpenFile::new(Box::new(Socket::new()))
}
