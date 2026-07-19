use alloc::vec::Vec;

use object::{
    Endianness, elf,
    read::elf::{ElfFile64, FileHeader, ProgramHeader},
};
use roxy_memory::UserAddress;

use crate::{ElfError, LoadType, ProgramHeaders};

pub(super) fn interpreter(
    file: &ElfFile64<'_, Endianness>,
    load_type: LoadType,
) -> Result<Option<Vec<u8>>, ElfError> {
    let mut path = None;

    for header in file.elf_program_headers() {
        let Some(candidate) = header
            .interpreter(file.endian(), file.data())
            .map_err(|_| ElfError::InvalidInterpreter)?
        else {
            continue;
        };

        if path.is_some() || !matches!(load_type, LoadType::Executable) || candidate.is_empty() {
            return Err(ElfError::InvalidInterpreter);
        }

        path = Some(candidate.to_vec());
    }

    Ok(path)
}

pub(super) fn program_headers(
    file: &ElfFile64<'_, Endianness>,
    bias: u64,
) -> Result<ProgramHeaders, ElfError> {
    let header = file.elf_header();
    let endian = file.endian();
    let offset = header.e_phoff(endian);
    let entry_size = header.e_phentsize(endian);
    let count = header.e_phnum(endian);

    if count == 0 {
        return Err(ElfError::InvalidProgramHeaders);
    }

    let size = u64::from(entry_size)
        .checked_mul(u64::from(count))
        .ok_or(ElfError::InvalidProgramHeaders)?;
    let address = header_address(file, offset, size)?
        .checked_add(bias)
        .and_then(UserAddress::new)
        .ok_or(ElfError::InvalidProgramHeaders)?;

    Ok(ProgramHeaders {
        address,
        entry_size,
        count,
    })
}

fn header_address(
    file: &ElfFile64<'_, Endianness>,
    offset: u64,
    size: u64,
) -> Result<u64, ElfError> {
    let endian = file.endian();

    for header in file.elf_program_headers() {
        if header.p_type(endian) != elf::PT_LOAD {
            continue;
        }

        let segment_offset = header.p_offset(endian);
        let file_size = header.p_filesz(endian);

        let Some(relative) = offset.checked_sub(segment_offset) else {
            continue;
        };

        let Some(end) = relative.checked_add(size) else {
            continue;
        };

        if end > file_size {
            continue;
        }

        return header
            .p_vaddr(endian)
            .checked_add(relative)
            .ok_or(ElfError::InvalidProgramHeaders);
    }

    Err(ElfError::InvalidProgramHeaders)
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_test::kernel_test;
    use roxy_vm::AddrSpace;

    use crate::{
        LoadType, load,
        test_utils::{HEADER_SIZE, ProgramHeader, image, program_header, write_u64},
    };

    kernel_test!("roxy-elf::parse-interpreter", parses_interpreter, {
        let mut image = image(2);
        let path = b"/usr/lib/ld.so\0";

        image[0x1800..0x1800 + path.len()].copy_from_slice(path);
        write_u64(&mut image, HEADER_SIZE + 32, 0x1900);
        program_header(
            &mut image,
            1,
            ProgramHeader {
                kind: 3,
                offset: 0x1800,
                address: 0,
                file_size: path.len() as u64,
                memory_size: path.len() as u64,
                flags: 4,
            },
        );

        let loaded = load(&mut AddrSpace::new().unwrap(), &image, LoadType::Executable).unwrap();

        assert_eq!(
            loaded.interpreter.as_deref(),
            Some(b"/usr/lib/ld.so".as_slice())
        );
    });
}
