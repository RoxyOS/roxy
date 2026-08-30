use alloc::{boxed::Box, sync::Arc};

use roxy_fd::OpenFile;
use roxy_fd::StatusFlags;

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
        {
            let end = OpenFile::new(Box::new(Socket::connected(connection.clone(), Side::First)));
            end.set_status_flags(StatusFlags::READ_WRITE);
            end
        },
        {
            let end = OpenFile::new(Box::new(Socket::connected(connection, Side::Second)));
            end.set_status_flags(StatusFlags::READ_WRITE);
            end
        },
    )
}

/// Creates one unconnected Unix stream socket.
#[must_use]
pub fn socket() -> Arc<OpenFile> {
    let file = OpenFile::new(Box::new(Socket::new()));
    file.set_status_flags(StatusFlags::READ_WRITE);
    file
}
