use bitflags::bitflags;
use roxy_process::MemoryError;

use crate::{Syscall, SyscallResult, errno::Errno, numbers::SyscallNumber};

pub(super) const SYSCALL: Syscall = Syscall::new(SyscallNumber::VmMap, handle);

const ANONYMOUS_FD: u64 = u64::MAX;

bitflags! {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct MapProtection: u64 {
        const READ = 0x1;
        const WRITE = 0x2;
        const EXECUTE = 0x4;
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct MapFlags: u64 {
        const SHARED = 0x1;
        const PRIVATE = 0x2;
        const FIXED = 0x10;
        const ANONYMOUS = 0x20;
    }
}

const READ_WRITE: MapProtection = MapProtection::READ.union(MapProtection::WRITE);
const REQUIRED_FLAGS: MapFlags = MapFlags::PRIVATE.union(MapFlags::ANONYMOUS);
const SUPPORTED_FLAGS: MapFlags = REQUIRED_FLAGS;

struct VmMapRequest {
    size: usize,
    protection: MapProtection,
    flags: MapFlags,
    file_descriptor: u64,
    offset: u64,
}

impl VmMapRequest {
    fn parse(arguments: [u64; 6]) -> Result<Self, Errno> {
        Ok(Self {
            size: usize::try_from(arguments[1]).map_err(|_| Errno::Invalid)?,
            protection: MapProtection::from_bits_retain(arguments[2]),
            flags: MapFlags::from_bits_retain(arguments[3]),
            file_descriptor: arguments[4],
            offset: arguments[5],
        })
    }

    fn validate(&self) -> Result<(), Errno> {
        if self.size == 0 {
            return Err(Errno::Invalid);
        }

        self.protection.validate()?;
        self.flags.validate()?;

        if self.file_descriptor != ANONYMOUS_FD || self.offset != 0 {
            return Err(Errno::Invalid);
        }

        Ok(())
    }
}

impl MapProtection {
    fn validate(self) -> Result<(), Errno> {
        let unknown = self.bits() & !Self::all().bits();

        if unknown != 0 {
            return Err(unsupported("vm_map.protection.unknown", unknown));
        }

        if self != READ_WRITE {
            return Err(unsupported("vm_map.protection", self.bits()));
        }

        Ok(())
    }
}

impl MapFlags {
    fn validate(self) -> Result<(), Errno> {
        let unknown = self.bits() & !Self::all().bits();

        if unknown != 0 {
            return Err(unsupported("vm_map.flags.unknown", unknown));
        }

        let unsupported_flags = self.difference(SUPPORTED_FLAGS);

        if !unsupported_flags.is_empty() {
            return Err(unsupported("vm_map.flags", unsupported_flags.bits()));
        }

        if !self.contains(REQUIRED_FLAGS) {
            return Err(unsupported("vm_map.flags.missing", self.bits()));
        }

        Ok(())
    }
}

fn handle(arguments: [u64; 6]) -> SyscallResult {
    let request = VmMapRequest::parse(arguments)?;

    request.validate()?;

    // Only supports anon mapping for now
    let address = roxy_process::allocate_anonymous(request.size).map_err(map_memory_error)?;

    Ok(address.as_u64())
}

fn map_memory_error(error: MemoryError) -> Errno {
    match error {
        MemoryError::InvalidRange | MemoryError::PartialUnmap => Errno::Invalid,
        MemoryError::OutOfMemory => Errno::NoMem,
        MemoryError::Fault => Errno::Fault,
    }
}

fn unsupported(operation: &str, argument: u64) -> Errno {
    crate::unsupported::unsupported_argument(operation, argument, Errno::NotSupported)
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_test::kernel_test;

    use super::{MapFlags, MapProtection, VmMapRequest};
    use crate::errno::Errno;

    kernel_test!("roxy-syscall::vm-map-request", vm_map_request, {
        let request = VmMapRequest::parse([0, 4096, 0x3, 0x22, u64::MAX, 0]).unwrap();

        assert_eq!(
            request.protection,
            MapProtection::READ | MapProtection::WRITE
        );
        assert_eq!(request.flags, MapFlags::PRIVATE | MapFlags::ANONYMOUS);
        assert_eq!(request.validate(), Ok(()));

        let empty = VmMapRequest::parse([0, 0, 0x3, 0x22, u64::MAX, 0]).unwrap();
        assert_eq!(empty.validate(), Err(Errno::Invalid));
    });
}
