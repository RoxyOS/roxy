#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Permissions {
    ReadOnly,
    ReadWrite,
    ReadExecute,
}

impl Permissions {
    pub(super) const fn writable(self) -> bool {
        matches!(self, Self::ReadWrite)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VmError {
    InvalidRange,
    AddressInUse,
    OutOfMemory,
    NotMapped,
    MappingFailed,
    PermissionDenied,
}
