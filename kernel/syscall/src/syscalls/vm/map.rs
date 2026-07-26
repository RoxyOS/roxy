use bitflags::bitflags;
use roxy_memory::UserAddress;
use roxy_process::MemoryError;

use super::MemoryProtection;
use crate::{
    SyscallResult,
    args::{Nullable, SyscallArg},
    errno::Errno,
    numbers::SyscallNumber,
    syscall,
};

syscall!(SyscallNumber::VmMap, handle(address: u64, size: usize => Invalid, protection: u64, flags: u64, file_descriptor: u64, offset: u64));

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MapPlacement {
    Anywhere,
    Fixed(UserAddress),
}

struct VmMapRequest {
    placement: MapPlacement,
    size: usize,
}

fn handle(
    address: u64,
    size: usize,
    protection: u64,
    flags: u64,
    file_descriptor: u64,
    offset: u64,
) -> SyscallResult {
    let arguments =
        VmMapArguments::parse(address, size, protection, flags, file_descriptor, offset)?;

    let request = arguments.validate()?;

    let address = request.execute()?;

    Ok(address.as_u64())
}

impl VmMapRequest {
    fn execute(self) -> Result<UserAddress, Errno> {
        match self.placement {
            MapPlacement::Anywhere => roxy_process::allocate_anonymous(self.size)
                .map_err(|error| map_memory_error(error, 0)),
            MapPlacement::Fixed(address) => roxy_process::allocate_anonymous_at(address, self.size)
                .map_err(|error| map_memory_error(error, address.as_u64())),
        }
    }
}

struct VmMapArguments {
    address: Option<UserAddress>,
    size: usize,
    protection: u64,
    flags: u64,
    file_descriptor: u64,
    offset: u64,
}

bitflags! {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct MapFlags: u64 {
        const SHARED = 0x1;
        const PRIVATE = 0x2;
        const FIXED = 0x10;
        const ANONYMOUS = 0x20;
    }
}

const ANONYMOUS_FD: u64 = u64::MAX;
const REQUIRED_FLAGS: MapFlags = MapFlags::PRIVATE.union(MapFlags::ANONYMOUS);
const SUPPORTED_FLAGS: MapFlags = REQUIRED_FLAGS.union(MapFlags::FIXED);

impl VmMapArguments {
    fn parse(
        address: u64,
        size: usize,
        protection: u64,
        flags: u64,
        file_descriptor: u64,
        offset: u64,
    ) -> Result<Self, Errno> {
        Ok(Self {
            address: Nullable::<UserAddress>::parse(address, Errno::Invalid)?.into_option(),
            size,
            protection,
            flags,
            file_descriptor,
            offset,
        })
    }

    fn validate(self) -> Result<VmMapRequest, Errno> {
        if self.size == 0 {
            return Err(Errno::Invalid);
        }

        MemoryProtection::validate_mapping(self.protection)?;
        let flags = MapFlags::parse(self.flags, Errno::Invalid)?;

        validate_anonymous_source(self.file_descriptor, self.offset)?;

        let placement = parse_placement(self.address, flags)?;

        Ok(VmMapRequest {
            placement,
            size: self.size,
        })
    }
}

impl SyscallArg for MapFlags {
    fn parse(raw: u64, _error: Errno) -> Result<Self, Errno> {
        let flags = Self::from_bits_retain(raw);
        let unknown = raw & !Self::all().bits();

        if unknown != 0 {
            return Err(unsupported("vm_map.flags.unknown", unknown));
        }

        if flags != REQUIRED_FLAGS && flags != SUPPORTED_FLAGS {
            return Err(unsupported("vm_map.flags", flags.bits()));
        }

        Ok(flags)
    }
}

fn parse_placement(address: Option<UserAddress>, flags: MapFlags) -> Result<MapPlacement, Errno> {
    match (flags.contains(MapFlags::FIXED), address) {
        (false, None) => Ok(MapPlacement::Anywhere),
        (true, Some(address)) => Ok(MapPlacement::Fixed(address)),
        _ => Err(unsupported("vm_map.fixed-address", flags.bits())),
    }
}

fn validate_anonymous_source(file_descriptor: u64, offset: u64) -> Result<(), Errno> {
    if file_descriptor != ANONYMOUS_FD || offset != 0 {
        return Err(unsupported("vm_map.file", file_descriptor));
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

    use super::{MapPlacement, VmMapArguments};

    kernel_test!("roxy-syscall::vm-map-requests", requests, {
        let fixed = VmMapArguments::parse(0x41_0000, 4096, 0x3, 0x32, u64::MAX, 0)
            .unwrap()
            .validate()
            .unwrap();
        let anywhere = VmMapArguments::parse(0, 4096, 0x3, 0x22, u64::MAX, 0)
            .unwrap()
            .validate()
            .unwrap();

        assert_eq!(
            fixed.placement,
            MapPlacement::Fixed(UserAddress::new(0x41_0000).unwrap())
        );
        assert_eq!(anywhere.placement, MapPlacement::Anywhere);
    });
}
