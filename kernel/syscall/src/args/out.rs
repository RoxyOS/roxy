use core::marker::PhantomData;

use roxy_memory::UserAddress;

use super::{SyscallArg, user_memory};
use crate::errno::Errno;

pub(crate) struct Out<T: ?Sized> {
    address: UserAddress,
    output: PhantomData<fn() -> T>,
}

impl<T: ?Sized> Clone for Out<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: ?Sized> Copy for Out<T> {}

impl<T> Out<T> {
    pub(crate) fn validate(self) -> Result<(), Errno> {
        user_memory::validate_writable(self.address, core::mem::size_of::<T>())
    }

    /// Writes `value` into this output slot in the current address space.
    ///
    /// # Safety
    ///
    /// `T` must have a stable layout with no implicit padding, and every byte in its
    /// representation must be initialized.
    pub(crate) unsafe fn write(self, value: &T) -> Result<(), Errno> {
        // SAFETY: The caller guarantees that value satisfies the object representation contract.
        unsafe { user_memory::write(self.address, value) }
    }
}

impl<T: ?Sized> SyscallArg for Out<T> {
    fn parse(raw: u64, error: Errno) -> Result<Self, Errno> {
        let address = UserAddress::new(raw).ok_or(error)?;

        Ok(Self {
            address,
            output: PhantomData,
        })
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_test::kernel_test;

    use super::{Out, SyscallArg};
    use crate::errno::Errno;

    struct Record;

    kernel_test!("roxy-syscall::out-argument", output_argument, {
        assert!(Out::<Record>::parse(0, Errno::Fault).is_err_and(|error| error == Errno::Fault));
        assert!(Out::<Record>::parse(0x40_0000, Errno::Fault).is_ok());
    });
}
