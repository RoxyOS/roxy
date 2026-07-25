use crate::OpenFile;

pub use roxy_tty_types::{ApplyWhen, LocalFlags, Termios, WindowSize};

#[derive(Debug)]
pub enum IoctlRequest<'a> {
    GetTermios(&'a mut Termios),
    SetTermios { when: ApplyWhen, termios: Termios },
    GetWindowSize(&'a mut WindowSize),
    SetWindowSize(WindowSize),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IoctlError {
    NotTty,
    Unsupported {
        operation: &'static str,
        argument: u64,
    },
}

impl OpenFile {
    /// Dispatches an ioctl request while holding the serialized open-file state.
    ///
    /// # Errors
    ///
    /// Returns the underlying object's ioctl error.
    pub fn ioctl(&self, request: IoctlRequest<'_>) -> Result<(), IoctlError> {
        let mut state = self.state.lock();

        state.object.ioctl(request)
    }
}
