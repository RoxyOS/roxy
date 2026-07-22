use core::num::NonZeroUsize;

use object::SegmentFlags;
use roxy_memory::{PAGE_SIZE, UserAddress, UserPage};
use roxy_vm::{AddrSpace, Permissions, UserRegion, VmError};

use crate::ElfError;

const PF_X: u32 = 1;
const PF_W: u32 = 2;

#[derive(Clone, Copy)]
pub(super) struct LoadFlags {
    writable: bool,
    pub executable: bool,
}

impl LoadFlags {
    pub(super) fn permissions(self) -> Permissions {
        match (self.writable, self.executable) {
            (true, false) => Permissions::ReadWrite,
            (false, true) => Permissions::ReadExecute,
            (false, false) => Permissions::ReadOnly,
            (true, true) => unreachable!(),
        }
    }
}

pub(super) struct SegmentMapping<'data> {
    pub address: UserAddress,
    pub memory_size: usize,
    pub region: UserRegion,
    pub flags: LoadFlags,
    pub data: &'data [u8],
}

impl SegmentMapping<'_> {
    pub(super) fn new(address: u64, size: u64, flags: LoadFlags) -> Result<Option<Self>, ElfError> {
        if size == 0 {
            return Ok(None);
        }

        let address = UserAddress::new(address).ok_or(ElfError::InvalidSegment)?;
        let last = address
            .checked_add(size.checked_sub(1).ok_or(ElfError::InvalidSegment)?)
            .ok_or(ElfError::InvalidSegment)?;
        let start = UserPage::containing(address);
        let end = UserPage::containing(last);
        let page_count = page_count(start, end)?;
        let memory_size = usize::try_from(size).map_err(|_| ElfError::InvalidSegment)?;
        let region = UserRegion::new(start, page_count).ok_or(ElfError::InvalidSegment)?;

        Ok(Some(Self {
            address,
            memory_size,
            region,
            flags,
            data: &[],
        }))
    }

    pub(super) fn contains(&self, address: u64) -> bool {
        let start = self.address.as_u64();
        start <= address && address - start < u64::try_from(self.memory_size).unwrap()
    }

    pub(super) fn overlaps(&self, other: &Self) -> bool {
        let start = self.region.start.start_address().as_u64();
        let end = start + u64::try_from(self.region.page_count.get()).unwrap() * PAGE_SIZE;
        let other_start = other.region.start.start_address().as_u64();
        let other_end =
            other_start + u64::try_from(other.region.page_count.get()).unwrap() * PAGE_SIZE;
        start < other_end && other_start < end
    }
}

pub(super) fn segment_flags(flags: SegmentFlags) -> Result<LoadFlags, ElfError> {
    let SegmentFlags::Elf { p_flags } = flags else {
        return Err(ElfError::UnsupportedFormat);
    };

    let flags = LoadFlags {
        writable: p_flags & PF_W != 0,
        executable: p_flags & PF_X != 0,
    };

    if flags.writable && flags.executable {
        return Err(ElfError::WritableExecutableSegment);
    }

    Ok(flags)
}

pub(super) fn map_segment(
    addrspace: &mut AddrSpace,
    mapping: &SegmentMapping<'_>,
) -> Result<(), ElfError> {
    addrspace
        .map_zeroed(mapping.region, Permissions::ReadWrite)
        .map_err(vm_error)?;

    addrspace
        .write_bytes(mapping.address, mapping.data)
        .map_err(vm_error)?;

    addrspace
        .protect(
            mapping.region.start.start_address(),
            mapping.region.byte_len(),
            mapping.flags.permissions(),
        )
        .map_err(vm_error)
}

fn vm_error(error: VmError) -> ElfError {
    match error {
        VmError::AddressInUse => ElfError::OverlappingSegments,
        VmError::OutOfMemory => ElfError::OutOfMemory,
        VmError::InvalidRange
        | VmError::PartialUnmap
        | VmError::NotMapped
        | VmError::MappingFailed
        | VmError::PermissionDenied => ElfError::InvalidSegment,
    }
}

fn page_count(start: UserPage, end: UserPage) -> Result<NonZeroUsize, ElfError> {
    ((end.start_address().as_u64() - start.start_address().as_u64()) / PAGE_SIZE)
        .checked_add(1)
        .and_then(|count| usize::try_from(count).ok())
        .and_then(NonZeroUsize::new)
        .ok_or(ElfError::InvalidSegment)
}

#[cfg(feature = "kernel-test")]
mod tests {
    use object::SegmentFlags;
    use roxy_test::kernel_test;

    use super::{SegmentMapping, segment_flags};
    use crate::ElfError;

    kernel_test!("roxy-elf::reject-segment-overflow", reject_overflow, {
        let flags = segment_flags(SegmentFlags::Elf { p_flags: 4 }).unwrap();

        assert!(matches!(
            SegmentMapping::new(0x7fff_ffff_f000, 0x2000, flags),
            Err(ElfError::InvalidSegment)
        ));
    });

    kernel_test!("roxy-elf::reject-writable-code", reject_writable_code, {
        assert!(matches!(
            segment_flags(SegmentFlags::Elf { p_flags: 7 }),
            Err(ElfError::WritableExecutableSegment)
        ));
    });
}
