#![no_std]

extern crate alloc;

use alloc::vec::Vec;

use object::{
    Architecture, Endianness, Object, ObjectKind, ObjectSection, ObjectSegment, elf,
    read::elf::{ElfFile64, ProgramHeader},
};
use roxy_memory::UserAddress;
use roxy_vm::{AddrSpace, VmError};

use self::segment::{SegmentMapping, segment_flags};

mod segment;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LoadedElf {
    pub entry: UserAddress,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ElfError {
    InvalidImage,
    UnsupportedFormat,
    InvalidSegment,
    OverlappingSegments,
    WritableExecutableSegment,
    InvalidEntry,
    OutOfMemory,
}

/// Eagerly maps a validated executable image into an address space.
///
/// # Errors
///
/// Returns an error for unsupported or malformed ELF input, conflicting mappings, or allocation
/// failure.
pub fn load(addrspace: &mut AddrSpace, image: &[u8]) -> Result<LoadedElf, ElfError> {
    let file = ElfFile64::<Endianness>::parse(image).map_err(|_| ElfError::InvalidImage)?;
    validate_file(&file)?;

    let mut mappings = Vec::new();
    mappings
        .try_reserve(file.elf_program_headers().len())
        .map_err(|_| ElfError::OutOfMemory)?;

    let mut executable_entry = false;
    for segment in file.segments() {
        let flags = segment_flags(segment.flags())?;
        let Some(mut mapping) = SegmentMapping::new(segment.address(), segment.size(), flags)?
        else {
            continue;
        };

        mapping.data = segment.data().map_err(|_| ElfError::InvalidSegment)?;
        if mapping.data.len() > mapping.memory_size {
            return Err(ElfError::InvalidSegment);
        }

        if mappings.iter().any(|existing| mapping.overlaps(existing)) {
            return Err(ElfError::OverlappingSegments);
        }

        executable_entry |= mapping.contains(file.entry()) && flags.executable;
        mappings.push(mapping);
    }

    let entry = UserAddress::new(file.entry()).ok_or(ElfError::InvalidEntry)?;
    if !executable_entry {
        return Err(ElfError::InvalidEntry);
    }

    for mapping in &mappings {
        map_segment(addrspace, mapping)?;
    }

    Ok(LoadedElf { entry })
}

fn validate_file(file: &ElfFile64<'_>) -> Result<(), ElfError> {
    if file.architecture() != Architecture::X86_64
        || file.endianness() != Endianness::Little
        || file.kind() != ObjectKind::Executable
    {
        return Err(ElfError::UnsupportedFormat);
    }

    if file.section_by_name(".interp").is_some() || file.section_by_name(".dynamic").is_some() {
        return Err(ElfError::UnsupportedFormat);
    }

    if file
        .sections()
        .any(|section| section.relocations().next().is_some())
    {
        return Err(ElfError::UnsupportedFormat);
    }

    for header in file.elf_program_headers() {
        if matches!(
            header.p_type(file.endian()),
            elf::PT_INTERP | elf::PT_DYNAMIC
        ) {
            return Err(ElfError::UnsupportedFormat);
        }

        if header.p_filesz(file.endian()) > header.p_memsz(file.endian()) {
            return Err(ElfError::InvalidSegment);
        }
    }

    Ok(())
}

fn map_segment(addrspace: &mut AddrSpace, mapping: &SegmentMapping<'_>) -> Result<(), ElfError> {
    addrspace
        .map_zeroed(mapping.region, mapping.flags.permissions())
        .map_err(vm_error)?;

    addrspace
        .write_bytes(mapping.address, mapping.data)
        .map_err(vm_error)
}

fn vm_error(error: VmError) -> ElfError {
    match error {
        VmError::AddressInUse => ElfError::OverlappingSegments,
        VmError::OutOfMemory => ElfError::OutOfMemory,
        VmError::InvalidRange | VmError::NotMapped | VmError::MappingFailed => {
            ElfError::InvalidSegment
        }
    }
}

#[cfg(feature = "kernel-test")]
mod tests;
