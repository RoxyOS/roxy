use alloc::{boxed::Box, sync::Arc};

use roxy_fd::OpenFile;

mod buffer;
mod connection;
mod endpoint;
mod side;

use connection::Connection;
use endpoint::Endpoint;
use side::Side;

/// Creates two connected, bidirectional Unix stream endpoints.
#[must_use]
pub fn pair() -> (Arc<OpenFile>, Arc<OpenFile>) {
    let connection = Arc::new(Connection::new());

    (
        OpenFile::new(Box::new(Endpoint::new(connection.clone(), Side::First))),
        OpenFile::new(Box::new(Endpoint::new(connection, Side::Second))),
    )
}
