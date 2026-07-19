use roxy_memory::UserAddress;
use roxy_vm::VmError;

use crate::current_addrspace;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryError {
    InvalidRange,
    OutOfMemory,
    Fault,
}

/// Allocates zero-filled writable memory in the current process.
///
/// # Errors
///
/// Returns an error for invalid sizes, address-space exhaustion, or mapping failures.
pub fn allocate_anonymous(size: usize) -> Result<UserAddress, MemoryError> {
    current_addrspace()
        .map_err(|_| MemoryError::Fault)?
        .allocate_anonymous(size)
        .map_err(map_vm_error)
}

/// Frees one complete anonymous allocation in the current process.
///
/// # Errors
///
/// Returns an error unless the address and size exactly match a live allocation.
pub fn free_anonymous(address: UserAddress, size: usize) -> Result<(), MemoryError> {
    current_addrspace()
        .map_err(|_| MemoryError::Fault)?
        .free_anonymous(address, size)
        .map_err(map_vm_error)
}

fn map_vm_error(error: VmError) -> MemoryError {
    match error {
        VmError::InvalidRange | VmError::AddressInUse | VmError::NotMapped => {
            MemoryError::InvalidRange
        }
        VmError::OutOfMemory => MemoryError::OutOfMemory,
        VmError::MappingFailed | VmError::PermissionDenied => MemoryError::Fault,
    }
}
