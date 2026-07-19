use bitflags::bitflags;
use roxy_memory::UserAddress;
use roxy_process::MemoryError;

use super::MemoryProtection;
use crate::{Syscall, SyscallResult, errno::Errno, numbers::SyscallNumber};

pub(super) const SYSCALL: Syscall = Syscall::new(SyscallNumber::VmMap, handle);

const ANONYMOUS_FD: u64 = u64::MAX;

bitflags! {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct MapFlags: u64 {
        const SHARED = 0x1;
        const PRIVATE = 0x2;
        const FIXED = 0x10;
        const ANONYMOUS = 0x20;
    }
}

const REQUIRED_FLAGS: MapFlags = MapFlags::PRIVATE.union(MapFlags::ANONYMOUS);
const SUPPORTED_FLAGS: MapFlags = REQUIRED_FLAGS.union(MapFlags::FIXED);

struct VmMapRequest {
    address: Option<UserAddress>,
    size: usize,
    protection: u64,
    flags: MapFlags,
    file_descriptor: u64,
    offset: u64,
}

impl VmMapRequest {
    fn parse(arguments: [u64; 6]) -> Result<Self, Errno> {
        let address = match arguments[0] {
            0 => None,
            value => Some(UserAddress::new(value).ok_or(Errno::Invalid)?),
        };
        let size = usize::try_from(arguments[1]).map_err(|_| Errno::Invalid)?;

        Ok(Self {
            address,
            size,
            protection: arguments[2],
            flags: MapFlags::from_bits_retain(arguments[3]),
            file_descriptor: arguments[4],
            offset: arguments[5],
        })
    }

    fn validate(&self) -> Result<(), Errno> {
        if self.size == 0 {
            return Err(Errno::Invalid);
        }

        MemoryProtection::for_mapping(self.protection)?;
        validate_flags(self.flags)?;
        validate_file(self.file_descriptor, self.offset)?;

        if self.flags.contains(MapFlags::FIXED) != self.address.is_some() {
            return Err(unsupported("vm_map.fixed-address", self.flags.bits()));
        }

        Ok(())
    }
}

fn handle(arguments: [u64; 6]) -> SyscallResult {
    let request = VmMapRequest::parse(arguments)?;

    request.validate()?;

    let requested_address = request.address.map_or(0, UserAddress::as_u64);
    let address = match request.address {
        Some(address) => roxy_process::allocate_anonymous_at(address, request.size),
        None => roxy_process::allocate_anonymous(request.size),
    }
    .map_err(|error| map_memory_error(error, requested_address))?;

    Ok(address.as_u64())
}

fn validate_file(file_descriptor: u64, offset: u64) -> Result<(), Errno> {
    if file_descriptor != ANONYMOUS_FD || offset != 0 {
        return Err(unsupported("vm_map.file", file_descriptor));
    }

    Ok(())
}

fn validate_flags(flags: MapFlags) -> Result<(), Errno> {
    let unknown = flags.bits() & !MapFlags::all().bits();

    if unknown != 0 {
        return Err(unsupported("vm_map.flags.unknown", unknown));
    }

    let unsupported_flags = flags.difference(SUPPORTED_FLAGS);

    if !unsupported_flags.is_empty() || !flags.contains(REQUIRED_FLAGS) {
        return Err(unsupported("vm_map.flags", flags.bits()));
    }

    Ok(())
}

fn map_memory_error(error: MemoryError, address: u64) -> Errno {
    match error {
        MemoryError::AddressInUse => unsupported("vm_map.fixed-overlap", address),
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
    use roxy_memory::UserAddress;
    use roxy_test::kernel_test;

    use super::{MapFlags, VmMapRequest};

    kernel_test!("roxy-syscall::vm-map-fixed-request", fixed_request, {
        let request = VmMapRequest::parse([0x41_0000, 4096, 0x3, 0x32, u64::MAX, 0]).unwrap();

        assert_eq!(request.address, UserAddress::new(0x41_0000));
        assert!(request.flags.contains(MapFlags::FIXED));
        assert_eq!(request.validate(), Ok(()));
    });
}
