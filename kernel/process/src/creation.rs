use roxy_thread::{Thread, ThreadCreateError};
use roxy_vm::{AddrSpace, VmError};

use crate::{Process, ProcessError};

impl Process {
    /// Builds a single-thread process from an in-memory executable image.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid ELF image, address-space failure, or allocation failure.
    pub fn from_elf(image: &[u8]) -> Result<Self, ProcessError> {
        let mut addrspace = AddrSpace::new().map_err(process_vm_error)?;
        let loaded = roxy_elf::load(&mut addrspace, image).map_err(|error| match error {
            roxy_elf::ElfError::OutOfMemory => ProcessError::OutOfMemory,
            _ => ProcessError::InvalidElf,
        })?;

        let stack = addrspace.map_stack().map_err(process_vm_error)?;
        let main_thread = Thread::new_user(loaded.entry, stack.top).map_err(thread_error)?;

        Ok(Self {
            _addrspace: addrspace,
            _main_thread: main_thread,
        })
    }
}

fn process_vm_error(error: VmError) -> ProcessError {
    match error {
        VmError::OutOfMemory => ProcessError::OutOfMemory,
        VmError::InvalidRange
        | VmError::AddressInUse
        | VmError::NotMapped
        | VmError::MappingFailed => ProcessError::InvalidAddressSpace,
    }
}

fn thread_error(error: ThreadCreateError) -> ProcessError {
    match error {
        ThreadCreateError::OutOfMemory => ProcessError::OutOfMemory,
    }
}
