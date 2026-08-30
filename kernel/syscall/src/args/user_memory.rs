use core::{mem, slice};

use roxy_memory::UserAddress;
use roxy_vm::AddrSpaceHandle;

use crate::errno::Errno;

pub(super) fn validate_writable(address: UserAddress, size: usize) -> Result<(), Errno> {
    let addrspace = current_addrspace()?;

    addrspace
        .validate_writable(address, size)
        .map_err(|_| Errno::Fault)
}

/// Copies userspace bytes into an initialized value.
///
/// # Safety
///
/// `T` must have a stable layout with no implicit padding, contain no references or invalid bit
/// patterns, and accept every possible userspace-supplied byte pattern.
pub(crate) unsafe fn read<T>(address: UserAddress, value: &mut T) -> Result<(), Errno> {
    let addrspace = current_addrspace()?;

    // SAFETY: The caller guarantees that the complete T representation accepts arbitrary bytes,
    // and the byte slice stays within the uniquely borrowed value.
    let bytes = unsafe {
        slice::from_raw_parts_mut(core::ptr::from_mut(value).cast::<u8>(), mem::size_of::<T>())
    };

    addrspace
        .read_bytes(address, bytes)
        .map_err(|_| Errno::Fault)
}

/// Copies an initialized value to userspace.
///
/// # Safety
///
/// `T` must have a stable layout with no implicit padding, and every byte in its representation
/// must be initialized.
pub(crate) unsafe fn write<T>(address: UserAddress, value: &T) -> Result<(), Errno> {
    let addrspace = current_addrspace()?;

    // SAFETY: The caller guarantees that every byte in T is initialized, and the byte slice does
    // not outlive the borrowed value.
    let bytes = unsafe {
        slice::from_raw_parts(core::ptr::from_ref(value).cast::<u8>(), mem::size_of::<T>())
    };

    addrspace
        .write_bytes(address, bytes)
        .map_err(|_| Errno::Fault)
}

/// Copies userspace bytes into initialized values.
///
/// # Safety
///
/// `T` must satisfy the safety requirements of `read`.
pub(crate) unsafe fn read_slice<T>(address: UserAddress, values: &mut [T]) -> Result<(), Errno> {
    let addrspace = current_addrspace()?;

    // SAFETY: The caller guarantees that each complete T representation accepts arbitrary bytes,
    // and the byte slice stays within the uniquely borrowed values.
    let bytes = unsafe {
        slice::from_raw_parts_mut(values.as_mut_ptr().cast::<u8>(), mem::size_of_val(values))
    };

    addrspace
        .read_bytes(address, bytes)
        .map_err(|_| Errno::Fault)
}

/// Copies initialized values to userspace.
///
/// # Safety
///
/// `T` must satisfy the safety requirements of `write`.
pub(crate) unsafe fn write_slice<T>(address: UserAddress, values: &[T]) -> Result<(), Errno> {
    let addrspace = current_addrspace()?;

    // SAFETY: The caller guarantees that every byte in each T is initialized, and the byte slice
    // does not outlive the borrowed values.
    let bytes =
        unsafe { slice::from_raw_parts(values.as_ptr().cast::<u8>(), mem::size_of_val(values)) };

    addrspace
        .write_bytes(address, bytes)
        .map_err(|_| Errno::Fault)
}

fn current_addrspace() -> Result<AddrSpaceHandle, Errno> {
    roxy_process::current_addrspace().map_err(|_| Errno::Fault)
}
