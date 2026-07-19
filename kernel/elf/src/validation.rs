use object::{
    Architecture, Endianness, Object, ObjectKind,
    read::elf::{ElfFile64, ProgramHeader},
};

use crate::{ElfError, LoadType};

pub(super) fn validate(
    file: &ElfFile64<'_, Endianness>,
    load_type: LoadType,
) -> Result<(), ElfError> {
    let expected = match load_type {
        LoadType::Executable => ObjectKind::Executable,
        LoadType::Interpreter { .. } => ObjectKind::Dynamic,
    };

    if file.architecture() != Architecture::X86_64
        || file.endianness() != Endianness::Little
        || file.kind() != expected
    {
        return Err(ElfError::UnsupportedFormat);
    }

    for header in file.elf_program_headers() {
        if header.p_filesz(file.endian()) > header.p_memsz(file.endian()) {
            return Err(ElfError::InvalidSegment);
        }
    }

    Ok(())
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_test::kernel_test;
    use roxy_vm::AddrSpace;

    use crate::{
        ElfError, LoadType, load,
        test_utils::{HEADER_SIZE, image, write_u64},
    };

    kernel_test!("roxy-elf::reject-format", reject_format, {
        for (offset, value) in [(4, 1), (5, 2), (16, 3), (18, 3)] {
            let mut image = image(1);

            image[offset] = value;
            assert!(load(&mut AddrSpace::new().unwrap(), &image, LoadType::Executable).is_err());
        }
    });

    kernel_test!("roxy-elf::reject-filesz", reject_filesz, {
        let mut image = image(1);

        write_u64(&mut image, HEADER_SIZE + 32, 0x3000);

        assert_eq!(load_error(&image), ElfError::InvalidSegment);
    });

    fn load_error(image: &[u8]) -> ElfError {
        load(&mut AddrSpace::new().unwrap(), image, LoadType::Executable).unwrap_err()
    }
}
