use core::mem::{align_of, offset_of, size_of};

use roxy_fb_types::{FbBitfield, FbFixedInfo, FbVarInfo};

use crate::{args::Out, errno::Errno};

/// Linux `struct fb_var_screeninfo` as seen by `x86_64` userspace.
///
/// The layout mirrors `include/uapi/linux/fb.h` for the `x86_64` personality: every field is a
/// 32-bit integer and the record is 160 bytes with no padding.
#[repr(C)]
pub(super) struct FbVarScreenInfoAbi {
    xres: u32,
    yres: u32,
    xres_virtual: u32,
    yres_virtual: u32,
    xoffset: u32,
    yoffset: u32,
    bits_per_pixel: u32,
    grayscale: u32,
    red: FbBitfieldAbi,
    green: FbBitfieldAbi,
    blue: FbBitfieldAbi,
    transp: FbBitfieldAbi,
    nonstd: u32,
    activate: u32,
    height: u32,
    width: u32,
    accel_flags: u32,
    pixclock: u32,
    left_margin: u32,
    right_margin: u32,
    upper_margin: u32,
    lower_margin: u32,
    hsync_len: u32,
    vsync_len: u32,
    sync: u32,
    vmode: u32,
    rotate: u32,
    colorspace: u32,
    reserved: [u32; 4],
}

const _: () = assert!(size_of::<FbVarScreenInfoAbi>() == 160);
const _: () = assert!(align_of::<FbVarScreenInfoAbi>() == 4);
const _: () = assert!(offset_of!(FbVarScreenInfoAbi, xres) == 0);
const _: () = assert!(offset_of!(FbVarScreenInfoAbi, yres) == 4);
const _: () = assert!(offset_of!(FbVarScreenInfoAbi, bits_per_pixel) == 24);
const _: () = assert!(offset_of!(FbVarScreenInfoAbi, red) == 32);
const _: () = assert!(offset_of!(FbVarScreenInfoAbi, reserved) == 144);

/// Linux `struct fb_bitfield` as seen by `x86_64` userspace.
#[repr(C)]
#[derive(Clone, Copy)]
struct FbBitfieldAbi {
    offset: u32,
    length: u32,
    msb_right: u32,
}

const _: () = assert!(size_of::<FbBitfieldAbi>() == 12);
const _: () = assert!(align_of::<FbBitfieldAbi>() == 4);

/// Linux `struct fb_fix_screeninfo` as seen by `x86_64` userspace.
///
/// The layout mirrors `include/uapi/linux/fb.h`: two `unsigned long` fields force 8-byte
/// alignment and the record totals 80 bytes on `x86_64`.
#[repr(C)]
pub(super) struct FbFixScreenInfoAbi {
    id: [u8; 16],
    smem_start: u64,
    smem_len: u32,
    fb_type: u32,
    type_aux: u32,
    visual: u32,
    xpanstep: u16,
    ypanstep: u16,
    ywrapstep: u16,
    line_length: u32,
    mmio_start: u64,
    mmio_len: u32,
    accel: u32,
    capabilities: u16,
    reserved: [u16; 2],
}

const _: () = assert!(size_of::<FbFixScreenInfoAbi>() == 80);
const _: () = assert!(align_of::<FbFixScreenInfoAbi>() == 8);
const _: () = assert!(offset_of!(FbFixScreenInfoAbi, id) == 0);
const _: () = assert!(offset_of!(FbFixScreenInfoAbi, smem_start) == 16);
const _: () = assert!(offset_of!(FbFixScreenInfoAbi, smem_len) == 24);
const _: () = assert!(offset_of!(FbFixScreenInfoAbi, visual) == 36);
const _: () = assert!(offset_of!(FbFixScreenInfoAbi, line_length) == 48);
const _: () = assert!(offset_of!(FbFixScreenInfoAbi, mmio_start) == 56);
const _: () = assert!(offset_of!(FbFixScreenInfoAbi, reserved) == 74);

pub(super) fn write_var_screen_info(
    output: Out<FbVarScreenInfoAbi>,
    info: FbVarInfo,
) -> Result<(), Errno> {
    let abi = FbVarScreenInfoAbi {
        xres: info.xres,
        yres: info.yres,
        xres_virtual: info.xres_virtual,
        yres_virtual: info.yres_virtual,
        xoffset: 0,
        yoffset: 0,
        bits_per_pixel: info.bits_per_pixel,
        grayscale: 0,
        red: bitfield(info.red),
        green: bitfield(info.green),
        blue: bitfield(info.blue),
        transp: bitfield(FbBitfield {
            offset: 0,
            length: 0,
        }),
        nonstd: 0,
        activate: 0,
        height: 0,
        width: 0,
        accel_flags: 0,
        pixclock: 0,
        left_margin: 0,
        right_margin: 0,
        upper_margin: 0,
        lower_margin: 0,
        hsync_len: 0,
        vsync_len: 0,
        sync: 0,
        vmode: 0,
        rotate: 0,
        colorspace: 0,
        reserved: [0; 4],
    };

    // SAFETY: FbVarScreenInfoAbi contains only initialized integer fields without implicit
    // padding, so every byte of its object representation is defined.
    unsafe { output.write(&abi) }
}

pub(super) fn write_fix_screen_info(
    output: Out<FbFixScreenInfoAbi>,
    info: FbFixedInfo,
) -> Result<(), Errno> {
    let abi = FbFixScreenInfoAbi {
        id: info.id,
        smem_start: info.smem_start,
        smem_len: info.smem_len,
        fb_type: FB_TYPE_PACKED_PIXELS,
        type_aux: 0,
        visual: info.visual,
        xpanstep: 0,
        ypanstep: 0,
        ywrapstep: 0,
        line_length: info.line_length,
        mmio_start: 0,
        mmio_len: 0,
        accel: FB_ACCEL_NONE,
        capabilities: 0,
        reserved: [0; 2],
    };

    // SAFETY: FbFixScreenInfoAbi's checked repr(C) layout represents all padding explicitly and
    // initializes every field, so every byte of its object representation is defined.
    unsafe { output.write(&abi) }
}

fn bitfield(bitfield: FbBitfield) -> FbBitfieldAbi {
    FbBitfieldAbi {
        offset: bitfield.offset,
        length: bitfield.length,
        msb_right: 0,
    }
}

const FB_TYPE_PACKED_PIXELS: u32 = 0;
const FB_ACCEL_NONE: u32 = 0;
