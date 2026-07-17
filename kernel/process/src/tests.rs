use alloc::vec::Vec;

use roxy_memory::statistics;
use roxy_test::kernel_test;

use crate::Process;

kernel_test!(
    "roxy-process::construction-teardown",
    construction_teardown,
    {
        let baseline = statistics().allocated_frames;
        {
            let _process = Process::from_elf(&image()).unwrap();
            assert!(statistics().allocated_frames > baseline);
        }
        assert_eq!(statistics().allocated_frames, baseline);
    }
);

fn image() -> Vec<u8> {
    let mut image = alloc::vec![0; 0x1001];
    image[..8].copy_from_slice(&[0x7f, b'E', b'L', b'F', 2, 1, 1, 0]);
    write_u16(&mut image, 16, 2);
    write_u16(&mut image, 18, 62);
    write_u32(&mut image, 20, 1);
    write_u64(&mut image, 24, 0x40_0000);
    write_u64(&mut image, 32, 64);
    write_u16(&mut image, 52, 64);
    write_u16(&mut image, 54, 56);
    write_u16(&mut image, 56, 1);
    write_u32(&mut image, 64, 1);
    write_u32(&mut image, 68, 5);
    write_u64(&mut image, 72, 0x1000);
    write_u64(&mut image, 80, 0x40_0000);
    write_u64(&mut image, 96, 1);
    write_u64(&mut image, 104, 0x1000);
    write_u64(&mut image, 112, 0x1000);
    image[0x1000] = 0xc3;
    image
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
