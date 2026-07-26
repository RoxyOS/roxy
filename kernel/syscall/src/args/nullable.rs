use super::SyscallArg;
use crate::errno::Errno;

/// An ABI pointer argument that may be null.
pub(crate) enum Nullable<T> {
    Null,
    Value(T),
}

impl<T> Nullable<T> {
    #[must_use]
    pub(crate) fn into_option(self) -> Option<T> {
        match self {
            Self::Null => None,
            Self::Value(value) => Some(value),
        }
    }
}

impl<T: SyscallArg> SyscallArg for Nullable<T> {
    fn parse(raw: u64, error: Errno) -> Result<Self, Errno> {
        if raw == 0 {
            Ok(Self::Null)
        } else {
            T::parse(raw, error).map(Self::Value)
        }
    }
}
