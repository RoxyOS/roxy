use roxy_memory::UserAddress;
use roxy_vm::VmError;

use crate::current_addrspace;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryError {
    InvalidRange,
    AddressInUse,
    PartialUnmap,
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

/// Allocates zero-filled writable memory at an exact address in the current process.
///
/// # Errors
///
/// Returns an error for invalid or occupied ranges and allocation failures.
pub fn allocate_anonymous_at(
    address: UserAddress,
    size: usize,
) -> Result<UserAddress, MemoryError> {
    current_addrspace()
        .map_err(|_| MemoryError::Fault)?
        .allocate_anonymous_at(address, size)
        .map_err(map_vm_error)
}

/// Changes permissions across a mapped user range.
///
/// # Errors
///
/// Returns an error for invalid, unaligned, or unmapped ranges.
pub fn protect_memory(
    address: UserAddress,
    size: usize,
    permissions: roxy_vm::Permissions,
) -> Result<(), MemoryError> {
    current_addrspace()
        .map_err(|_| MemoryError::Fault)?
        .protect(address, size, permissions)
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

/// Unmaps one complete page-rounded anonymous allocation in the current process.
///
/// # Errors
///
/// Returns an error for invalid ranges, partial unmaps, or mapping failures.
pub fn unmap_anonymous(address: UserAddress, size: usize) -> Result<(), MemoryError> {
    current_addrspace()
        .map_err(|_| MemoryError::Fault)?
        .unmap_anonymous(address, size)
        .map_err(map_vm_error)
}

fn map_vm_error(error: VmError) -> MemoryError {
    match error {
        VmError::InvalidRange | VmError::NotMapped => MemoryError::InvalidRange,
        VmError::AddressInUse => MemoryError::AddressInUse,
        VmError::PartialUnmap => MemoryError::PartialUnmap,
        VmError::OutOfMemory => MemoryError::OutOfMemory,
        VmError::MappingFailed | VmError::PermissionDenied => MemoryError::Fault,
    }
}
