use alloc::vec::Vec;
use core::{
    marker::PhantomData,
    mem::{MaybeUninit, size_of},
};

use roxy_memory::UserAddress;

use super::user_memory;
use crate::errno::Errno;

pub(crate) struct Slice<T> {
    address: UserAddress,
    length: usize,
    element: PhantomData<fn() -> T>,
}

impl<T> Slice<T> {
    pub(crate) const fn new(address: UserAddress, length: usize) -> Self {
        Self {
            address,
            length,
            element: PhantomData,
        }
    }

    /// Copies this user slice into an owned vector.
    ///
    /// # Safety
    ///
    /// `T` must have a stable layout, contain no references or invalid bit patterns, and accept
    /// every possible userspace-supplied byte pattern.
    pub(crate) unsafe fn read(&self) -> Result<Vec<T>, Errno> {
        // SAFETY: The caller guarantees that T accepts every copied bit pattern.
        unsafe { self.read_with_limit(self.length) }
    }

    /// Copies at most `limit` elements from this user slice into an owned vector.
    ///
    /// # Safety
    ///
    /// `T` must satisfy the safety requirements of `read`.
    pub(crate) unsafe fn read_with_limit(&self, limit: usize) -> Result<Vec<T>, Errno> {
        let length = self.length.min(limit);
        let input = Self::new(self.address, length);

        input.validate()?;

        let mut values = Vec::<MaybeUninit<T>>::new();
        values.try_reserve_exact(length).map_err(|_| Errno::NoMem)?;

        // SAFETY: The allocation has capacity for length elements, and MaybeUninit<T> requires no
        // initialization before its length is exposed.
        unsafe { values.set_len(length) };

        // SAFETY: MaybeUninit<T> accepts arbitrary bytes and has T's object representation.
        unsafe { user_memory::read_slice(input.address, &mut values) }?;

        let pointer = values.as_mut_ptr().cast::<T>();
        let length = values.len();
        let capacity = values.capacity();
        core::mem::forget(values);

        // SAFETY: The caller guarantees that every copied element is a valid T, and this reuses
        // the same allocation, length, and capacity with T's layout.
        Ok(unsafe { Vec::from_raw_parts(pointer, length, capacity) })
    }

    /// Copies initialized values into this user slice.
    ///
    /// # Safety
    ///
    /// `T` must have a stable layout without implicit padding, and every byte in each value must
    /// be initialized.
    pub(crate) unsafe fn write(&self, values: &[T]) -> Result<(), Errno> {
        if values.len() > self.length {
            return Err(Errno::Invalid);
        }

        Self::new(self.address, values.len()).validate()?;

        // SAFETY: The caller guarantees that values satisfy the object representation contract.
        unsafe { user_memory::write_slice(self.address, values) }
    }

    pub(crate) fn skip(&self, elements: usize) -> Result<Self, Errno> {
        let length = self.length.checked_sub(elements).ok_or(Errno::Invalid)?;
        let byte_offset = Self::byte_length(elements)?;
        let address = self
            .address
            .checked_add(u64::try_from(byte_offset).map_err(|_| Errno::Fault)?)
            .ok_or(Errno::Fault)?;

        Ok(Self::new(address, length))
    }

    pub(crate) fn validate(&self) -> Result<(), Errno> {
        let byte_length = Self::byte_length(self.length)?;

        if let Some(last_offset) = byte_length.checked_sub(1) {
            self.address
                .checked_add(u64::try_from(last_offset).map_err(|_| Errno::Fault)?)
                .ok_or(Errno::Fault)?;
        }

        Ok(())
    }

    pub(crate) fn validate_writable(&self) -> Result<(), Errno> {
        self.validate()?;

        let addrspace = roxy_process::current_addrspace().map_err(|_| Errno::Fault)?;
        addrspace
            .validate_writable(self.address, Self::byte_length(self.length)?)
            .map_err(|_| Errno::Fault)
    }

    fn byte_length(length: usize) -> Result<usize, Errno> {
        if size_of::<T>() == 0 {
            return Err(Errno::Invalid);
        }

        length.checked_mul(size_of::<T>()).ok_or(Errno::Invalid)
    }
}
