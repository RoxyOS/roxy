#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Permissions {
    ReadOnly,
    ReadWrite,
    ReadExecute,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VmError {
    InvalidRange,
    AddressInUse,
    OutOfMemory,
    NotMapped,
    MappingFailed,
}
