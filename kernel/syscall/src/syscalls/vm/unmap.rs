use roxy_memory::UserAddress;
use roxy_process::MemoryError;

use crate::{SyscallResult, errno::Errno, numbers::SyscallNumber, syscall};

syscall!(SyscallNumber::VmUnmap, handle(address: UserAddress => Invalid, size: usize => Invalid));

struct VmUnmapRequest {
    address: UserAddress,
    size: usize,
}

impl VmUnmapRequest {
    fn execute(self) -> Result<(), Errno> {
        if self.size == 0 {
            return Err(Errno::Invalid);
        }

        // Only have anon mappings for now
        roxy_process::unmap_anonymous(self.address, self.size)
            .map_err(|error| map_memory_error(self.address, error))
    }
}

fn handle(address: UserAddress, size: usize) -> SyscallResult {
    let request = VmUnmapRequest { address, size };

    request.execute()?;

    Ok(0)
}

fn map_memory_error(address: UserAddress, error: MemoryError) -> Errno {
    match error {
        MemoryError::InvalidRange | MemoryError::AddressInUse => Errno::Invalid,
        MemoryError::PartialUnmap => crate::unsupported::unsupported_argument(
            "vm_unmap.partial",
            address.as_u64(),
            Errno::NotSupported,
        ),
        MemoryError::OutOfMemory => Errno::NoMem,
        MemoryError::Fault => Errno::Fault,
    }
}
