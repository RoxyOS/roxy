use bitflags::bitflags;
use roxy_fd::{Fd, MmapError};
use roxy_memory::{PhysicalAddress, UserAddress};
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MapSource {
    Anonymous,
    File { fd: Fd, offset: u64 },
}

struct VmMapRequest {
    placement: MapPlacement,
    source: MapSource,
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
        match (self.placement, self.source) {
            (MapPlacement::Anywhere, MapSource::Anonymous) => {
                roxy_process::allocate_anonymous(self.size)
                    .map_err(|error| map_memory_error(error, 0))
            }
            (MapPlacement::Fixed(address), MapSource::Anonymous) => {
                roxy_process::allocate_anonymous_at(address, self.size)
                    .map_err(|error| map_memory_error(error, address.as_u64()))
            }
            (placement, MapSource::File { fd, offset }) => self.map_file(placement, fd, offset),
        }
    }

    fn map_file(&self, placement: MapPlacement, fd: Fd, offset: u64) -> Result<UserAddress, Errno> {
        let file = roxy_process::current_open_file(fd).map_err(|_| Errno::BadFd)?;
        let target = file.mmap(self.size, offset).map_err(map_mmap_error)?;
        let physical = PhysicalAddress::new(target.physical_address).ok_or(Errno::Invalid)?;
        let permissions = roxy_vm::Permissions::ReadWrite;
        let result = match placement {
            MapPlacement::Anywhere => {
                roxy_process::map_physical(None, self.size, physical, permissions)
            }
            MapPlacement::Fixed(address) => {
                roxy_process::map_physical(Some(address), self.size, physical, permissions)
            }
        };

        result.map_err(|error| map_memory_error(error, 0))
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

        let placement = parse_placement(self.address, flags)?;
        let source = parse_source(self.file_descriptor, self.offset, flags)?;

        Ok(VmMapRequest {
            placement,
            source,
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

        let anonymous = flags.contains(MapFlags::ANONYMOUS);
        let valid = if anonymous {
            flags.contains(MapFlags::PRIVATE) && !flags.contains(MapFlags::SHARED)
        } else {
            flags.contains(MapFlags::SHARED) && !flags.contains(MapFlags::PRIVATE)
        };

        if !valid {
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

fn parse_source(file_descriptor: u64, offset: u64, flags: MapFlags) -> Result<MapSource, Errno> {
    if flags.contains(MapFlags::ANONYMOUS) {
        if file_descriptor != ANONYMOUS_FD || offset != 0 {
            return Err(unsupported("vm_map.file", file_descriptor));
        }

        return Ok(MapSource::Anonymous);
    }

    if file_descriptor == ANONYMOUS_FD {
        return Err(unsupported("vm_map.file", file_descriptor));
    }

    let fd = u32::try_from(file_descriptor)
        .map(Fd::new)
        .map_err(|_| Errno::BadFd)?;

    Ok(MapSource::File { fd, offset })
}

fn map_mmap_error(error: MmapError) -> Errno {
    match error {
        MmapError::Unsupported => crate::unsupported::unsupported_argument(
            "vm_map.file.unsupported",
            0,
            Errno::NotSupported,
        ),
        MmapError::InvalidArgument => Errno::Invalid,
    }
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
    use roxy_fd::Fd;
    use roxy_memory::UserAddress;
    use roxy_test::kernel_test;

    use super::{MapPlacement, MapSource, VmMapArguments};

    kernel_test!("roxy-syscall::vm-map-requests", requests, {
        let fixed = VmMapArguments::parse(0x41_0000, 4096, 0x3, 0x32, u64::MAX, 0)
            .unwrap()
            .validate()
            .unwrap();
        let anywhere = VmMapArguments::parse(0, 4096, 0x3, 0x22, u64::MAX, 0)
            .unwrap()
            .validate()
            .unwrap();
        let file = VmMapArguments::parse(0, 4096, 0x3, 0x1, 3, 0)
            .unwrap()
            .validate()
            .unwrap();

        assert_eq!(
            fixed.placement,
            MapPlacement::Fixed(UserAddress::new(0x41_0000).unwrap())
        );
        assert_eq!(anywhere.placement, MapPlacement::Anywhere);
        assert_eq!(fixed.source, MapSource::Anonymous);
        assert_eq!(
            file.source,
            MapSource::File {
                fd: Fd::new(3),
                offset: 0
            }
        );
    });
}
