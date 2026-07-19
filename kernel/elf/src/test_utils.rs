use alloc::vec::Vec;

pub(super) const HEADER_SIZE: usize = 64;
pub(super) const PROGRAM_HEADER_SIZE: usize = 56;
pub(super) const BASE: u64 = 0x40_0000;
pub(super) const ENTRY_OFFSET: u64 = 0x1000;
pub(super) const ENTRY: u64 = BASE + ENTRY_OFFSET;

#[derive(Clone, Copy)]
pub(super) struct ProgramHeader {
    pub kind: u32,
    pub offset: u64,
    pub address: u64,
    pub file_size: u64,
    pub memory_size: u64,
    pub flags: u32,
}

pub(super) fn image(program_headers: u16) -> Vec<u8> {
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
    program_header(
        &mut image,
        0,
        ProgramHeader {
            kind: 1,
            offset: 0,
            address: BASE,
            file_size: 0x1010,
            memory_size: 0x2000,
            flags: 5,
        },
    );
    image[0x1000] = 0xc3;

    image
}

pub(super) fn program_header(image: &mut [u8], index: usize, value: ProgramHeader) {
    let header = HEADER_SIZE + index * PROGRAM_HEADER_SIZE;

    write_u32(image, header, value.kind);
    write_u32(image, header + 4, value.flags);
    write_u64(image, header + 8, value.offset);
    write_u64(image, header + 16, value.address);
    write_u64(image, header + 32, value.file_size);
    write_u64(image, header + 40, value.memory_size);
    write_u64(image, header + 48, 0x1000);
}

pub(super) fn write_u16(image: &mut [u8], offset: usize, value: u16) {
    image[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

pub(super) fn write_u32(image: &mut [u8], offset: usize, value: u32) {
    image[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

pub(super) fn write_u64(image: &mut [u8], offset: usize, value: u64) {
    image[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}
