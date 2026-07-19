use alloc::vec::Vec;

use object::{Endianness, Object, ObjectSegment, read::elf::ElfFile64};
use roxy_memory::UserAddress;
use roxy_vm::AddrSpace;

use crate::{
    ElfError, LoadType, LoadedElf,
    metadata::{interpreter, program_headers},
    segment::{SegmentMapping, map_segment, segment_flags},
    validation::validate,
};

pub(super) fn load(
    addrspace: &mut AddrSpace,
    image: &[u8],
    load_type: LoadType,
) -> Result<LoadedElf, ElfError> {
    let file = ElfFile64::<Endianness>::parse(image).map_err(|_| ElfError::InvalidImage)?;

    validate(&file, load_type)?;

    let bias = load_type.bias();

    let mappings = mappings(&file, bias)?;
    let entry = entry(&file, bias, &mappings)?;
    let program_headers = program_headers(&file, bias)?;
    let interpreter = interpreter(&file, load_type)?;

    for mapping in &mappings {
        map_segment(addrspace, mapping)?;
    }

    Ok(LoadedElf {
        entry,
        base: bias,
        program_headers,
        interpreter,
    })
}

fn mappings<'data>(
    file: &ElfFile64<'data, Endianness>,
    bias: u64,
) -> Result<Vec<SegmentMapping<'data>>, ElfError> {
    let mut mappings = Vec::new();

    mappings
        .try_reserve(file.elf_program_headers().len())
        .map_err(|_| ElfError::OutOfMemory)?;

    for segment in file.segments() {
        let flags = segment_flags(segment.flags())?;
        let address = segment
            .address()
            .checked_add(bias)
            .ok_or(ElfError::InvalidSegment)?;
        let Some(mut mapping) = SegmentMapping::new(address, segment.size(), flags)? else {
            continue;
        };

        mapping.data = segment.data().map_err(|_| ElfError::InvalidSegment)?;

        if mapping.data.len() > mapping.memory_size {
            return Err(ElfError::InvalidSegment);
        }
        if mappings.iter().any(|existing| mapping.overlaps(existing)) {
            return Err(ElfError::OverlappingSegments);
        }

        mappings.push(mapping);
    }

    Ok(mappings)
}

fn entry(
    file: &ElfFile64<'_, Endianness>,
    bias: u64,
    mappings: &[SegmentMapping<'_>],
) -> Result<UserAddress, ElfError> {
    let raw = file
        .entry()
        .checked_add(bias)
        .ok_or(ElfError::InvalidEntry)?;
    let entry = UserAddress::new(raw).ok_or(ElfError::InvalidEntry)?;
    let executable = mappings
        .iter()
        .any(|mapping| mapping.contains(raw) && mapping.flags.executable);

    executable.then_some(entry).ok_or(ElfError::InvalidEntry)
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_memory::UserAddress;
    use roxy_test::kernel_test;
    use roxy_vm::AddrSpace;

    use crate::{
        ElfError, LoadType, load,
        test_utils::{
            BASE, ENTRY, ENTRY_OFFSET, HEADER_SIZE, ProgramHeader, image, program_header,
            write_u16, write_u64,
        },
    };

    kernel_test!("roxy-elf::load-valid", load_valid, {
        let image = image(1);
        let mut addrspace = AddrSpace::new().unwrap();
        let loaded = load(&mut addrspace, &image, LoadType::Executable).unwrap();

        assert_eq!(loaded.entry, UserAddress::new(ENTRY).unwrap());
        assert_eq!(
            loaded.program_headers.address,
            UserAddress::new(BASE + 64).unwrap()
        );

        let mut bytes = [0xff; 2];

        addrspace.read_bytes(loaded.entry, &mut bytes).unwrap();
        assert_eq!(bytes, [0xc3, 0]);
    });

    kernel_test!("roxy-elf::load-interpreter", load_interpreter, {
        let mut image = image(1);
        let base = UserAddress::new(0x20_0000_0000).unwrap();

        write_u16(&mut image, 16, 3);
        write_u64(&mut image, 24, ENTRY_OFFSET);
        write_u64(&mut image, HEADER_SIZE + 16, 0);

        let loaded = load(
            &mut AddrSpace::new().unwrap(),
            &image,
            LoadType::Interpreter { base },
        )
        .unwrap();

        assert_eq!(loaded.entry, base.checked_add(ENTRY_OFFSET).unwrap());
        assert_eq!(loaded.base, base.as_u64());
    });

    kernel_test!("roxy-elf::reject-overlap", reject_overlap, {
        let mut image = image(2);

        program_header(
            &mut image,
            1,
            ProgramHeader {
                kind: 1,
                offset: 0x2000,
                address: BASE + 0x1800,
                file_size: 1,
                memory_size: 0x1000,
                flags: 4,
            },
        );

        assert_eq!(load_error(&image), ElfError::OverlappingSegments);
    });

    kernel_test!("roxy-elf::reject-entry", reject_entry, {
        let mut image = image(1);

        write_u64(&mut image, 24, 0x50_0000);

        assert_eq!(load_error(&image), ElfError::InvalidEntry);
    });

    fn load_error(image: &[u8]) -> ElfError {
        load(&mut AddrSpace::new().unwrap(), image, LoadType::Executable).unwrap_err()
    }
}
