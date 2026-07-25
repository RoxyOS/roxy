use crate::OpenFile;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IoctlRequest {}

impl IoctlRequest {
    /// Parses a raw request number together with its request-specific argument.
    #[must_use]
    pub fn parse(_raw_request: u64, _raw_argument: u64) -> Option<Self> {
        None
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IoctlError {
    NotTty,
}

impl OpenFile {
    /// Dispatches an ioctl request while holding the serialized open-file state.
    ///
    /// # Errors
    ///
    /// Returns the underlying object's ioctl error.
    pub fn ioctl(&self, request: IoctlRequest) -> Result<u64, IoctlError> {
        let mut state = self.state.lock();

        state.object.ioctl(request)
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_test::kernel_test;

    use super::IoctlRequest;

    kernel_test!("roxy-fd::ioctl-parse", rejects_unknown_ioctl, {
        assert_eq!(IoctlRequest::parse(0x1234, 0x5678), None);
    });
}
