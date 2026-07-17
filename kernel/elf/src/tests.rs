use alloc::vec::Vec;

use roxy_memory::UserAddress;
use roxy_test::kernel_test;
use roxy_vm::AddrSpace;

use crate::{ElfError, load};

const HEADER_SIZE: usize = 64;
const PROGRAM_HEADER_SIZE: usize = 56;
const ENTRY: u64 = 0x40_0000;

kernel_test!("roxy-elf::load-valid", load_valid, {
    let image = image(1);
    let mut addrspace = AddrSpace::new().unwrap();
    let loaded = load(&mut addrspace, &image).unwrap();
    assert_eq!(loaded.entry, UserAddress::new(ENTRY).unwrap());
    let mut bytes = [0xff; 2];
    addrspace.read_bytes(loaded.entry, &mut bytes).unwrap();
    assert_eq!(bytes, [0xc3, 0]);
});

kernel_test!("roxy-elf::reject-format", reject_format, {
    for (offset, value) in [(4, 1), (5, 2), (16, 3), (18, 3)] {
        let mut image = image(1);
        image[offset] = value;
        assert!(load(&mut AddrSpace::new().unwrap(), &image).is_err());
    }
});

kernel_test!("roxy-elf::reject-filesz", reject_filesz, {
    let mut image = image(1);
    write_u64(&mut image, HEADER_SIZE + 32, 0x2000);
    assert_eq!(
        load(&mut AddrSpace::new().unwrap(), &image).unwrap_err(),
        ElfError::InvalidSegment
    );
});

kernel_test!("roxy-elf::reject-overflow", reject_overflow, {
    let mut image = image(1);
    write_u64(&mut image, HEADER_SIZE + 16, 0x7fff_ffff_f000);
    write_u64(&mut image, HEADER_SIZE + 40, 0x2000);
    assert_eq!(
        load(&mut AddrSpace::new().unwrap(), &image).unwrap_err(),
        ElfError::InvalidSegment
    );
});

kernel_test!("roxy-elf::reject-overlap", reject_overlap, {
    let mut image = image(2);
    program_header(&mut image, 1, 0x2000, 0x40_0800, 4);
    assert_eq!(
        load(&mut AddrSpace::new().unwrap(), &image).unwrap_err(),
        ElfError::OverlappingSegments
    );
});

kernel_test!("roxy-elf::reject-writable-code", reject_writable_code, {
    let mut image = image(1);
    write_u32(&mut image, HEADER_SIZE + 4, 7);
    assert_eq!(
        load(&mut AddrSpace::new().unwrap(), &image).unwrap_err(),
        ElfError::WritableExecutableSegment
    );
});

kernel_test!("roxy-elf::reject-entry", reject_entry, {
    let mut image = image(1);
    write_u64(&mut image, 24, 0x50_0000);
    assert_eq!(
        load(&mut AddrSpace::new().unwrap(), &image).unwrap_err(),
        ElfError::InvalidEntry
    );
});

fn image(program_headers: u16) -> Vec<u8> {
    let mut image = alloc::vec![0; 0x2010];
    image[..8].copy_from_slice(&[0x7f, b'E', b'L', b'F', 2, 1, 1, 0]);
    write_u16(&mut image, 16, 2);
    write_u16(&mut image, 18, 62);
    write_u32(&mut image, 20, 1);
    write_u64(&mut image, 24, ENTRY);
    write_u64(&mut image, 32, HEADER_SIZE as u64);
    write_u16(&mut image, 52, u16::try_from(HEADER_SIZE).unwrap());
    write_u16(&mut image, 54, u16::try_from(PROGRAM_HEADER_SIZE).unwrap());
    write_u16(&mut image, 56, program_headers);
    program_header(&mut image, 0, 0x1000, ENTRY, 5);
    image[0x1000] = 0xc3;
    image
}

fn program_header(image: &mut [u8], index: usize, offset: u64, address: u64, flags: u32) {
    let header = HEADER_SIZE + index * PROGRAM_HEADER_SIZE;
    write_u32(image, header, 1);
    write_u32(image, header + 4, flags);
    write_u64(image, header + 8, offset);
    write_u64(image, header + 16, address);
    write_u64(image, header + 32, 1);
    write_u64(image, header + 40, 0x1000);
    write_u64(image, header + 48, 0x1000);
    image[usize::try_from(offset).unwrap()] = 0xc3;
}

fn write_u16(image: &mut [u8], offset: usize, value: u16) {
    image[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(image: &mut [u8], offset: usize, value: u32) {
    image[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64(image: &mut [u8], offset: usize, value: u64) {
    image[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}
