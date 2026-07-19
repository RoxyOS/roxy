use roxy_memory::UserAddress;
use roxy_process::MemoryError;

use crate::{Syscall, SyscallResult, errno::Errno, numbers::SyscallNumber};

pub(super) const SYSCALL: Syscall = Syscall::new(SyscallNumber::VmUnmap, handle);

struct VmUnmapRequest {
    address: UserAddress,
    size: usize,
}

impl VmUnmapRequest {
    fn parse(arguments: [u64; 6]) -> Result<Self, Errno> {
        Ok(Self {
            address: UserAddress::new(arguments[0]).ok_or(Errno::Invalid)?,
            size: usize::try_from(arguments[1]).map_err(|_| Errno::Invalid)?,
        })
    }

    fn execute(self) -> Result<(), Errno> {
        if self.size == 0 {
            return Err(Errno::Invalid);
        }

        // Only have anon mappings for now
        roxy_process::unmap_anonymous(self.address, self.size)
            .map_err(|error| map_memory_error(self.address, error))
    }
}

fn handle(arguments: [u64; 6]) -> SyscallResult {
    let request = VmUnmapRequest::parse(arguments)?;

    request.execute()?;

    Ok(0)
}

fn map_memory_error(address: UserAddress, error: MemoryError) -> Errno {
    match error {
        MemoryError::InvalidRange => Errno::Invalid,
        MemoryError::PartialUnmap => crate::unsupported::unsupported_argument(
            "vm_unmap.partial",
            address.as_u64(),
            Errno::NotSupported,
        ),
        MemoryError::OutOfMemory => Errno::NoMem,
        MemoryError::Fault => Errno::Fault,
    }
}
