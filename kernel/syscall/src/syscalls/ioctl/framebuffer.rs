use roxy_fb_types::{FbBitfield, FbFixedInfo, FbVarInfo};
use roxy_fd::{IoctlRequest, OpenFile};
use roxy_memory::UserAddress;

use super::framebuffer_abi;
use crate::{
    args::{Out, SyscallArg},
    errno::Errno,
};

pub(super) const FBIOGET_VSCREENINFO: u64 = 0x4600;
pub(super) const FBIOPUT_VSCREENINFO: u64 = 0x4601;
pub(super) const FBIOGET_FSCREENINFO: u64 = 0x4602;

pub(super) fn get_var_screen_info(file: &OpenFile, address: UserAddress) -> Result<(), Errno> {
    let output = Out::<framebuffer_abi::FbVarScreenInfoAbi>::parse(address.as_u64(), Errno::Fault)?;
    output.validate()?;
    let mut info = FbVarInfo {
        xres: 0,
        yres: 0,
        xres_virtual: 0,
        yres_virtual: 0,
        bits_per_pixel: 0,
        red: empty_bitfield(),
        green: empty_bitfield(),
        blue: empty_bitfield(),
    };

    file.ioctl(IoctlRequest::FbGetVarInfo(&mut info))
        .map_err(super::execute::map_ioctl_error)?;
    framebuffer_abi::write_var_screen_info(output, info)
}

pub(super) fn set_var_screen_info(file: &OpenFile, address: UserAddress) -> Result<(), Errno> {
    let info = framebuffer_abi::read_var_screen_info(address)?;

    file.ioctl(IoctlRequest::FbSetVarInfo(info))
        .map_err(super::execute::map_ioctl_error)
}

pub(super) fn get_fix_screen_info(file: &OpenFile, address: UserAddress) -> Result<(), Errno> {
    let output = Out::<framebuffer_abi::FbFixScreenInfoAbi>::parse(address.as_u64(), Errno::Fault)?;
    output.validate()?;
    let mut info = FbFixedInfo {
        id: [0; 16],
        smem_start: 0,
        smem_len: 0,
        visual: 0,
        line_length: 0,
    };

    file.ioctl(IoctlRequest::FbGetFixedInfo(&mut info))
        .map_err(super::execute::map_ioctl_error)?;
    framebuffer_abi::write_fix_screen_info(output, info)
}

fn empty_bitfield() -> FbBitfield {
    FbBitfield {
        offset: 0,
        length: 0,
    }
}
