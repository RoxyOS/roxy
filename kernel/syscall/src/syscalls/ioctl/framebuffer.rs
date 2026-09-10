use roxy_fb_types::FbInfo;
use roxy_fd::{IoctlRequest, OpenFile};
use roxy_memory::UserAddress;

use super::framebuffer_abi;
use crate::{args::Out, args::SyscallArg, errno::Errno};

/// `ROXY_FRAMEBUFFER_GET_INFO`: report the framebuffer layout and pixel format.
///
/// This is the whole Roxy framebuffer request set. The layout is fixed by the boot loader, so
/// there is no mode-setting request: a client that wants to check whether a mode is usable
/// compares it against what this request reported. Pixel access happens through `mmap` with offset
/// zero, never through ioctl.
pub(super) const ROXY_FRAMEBUFFER_GET_INFO: u64 = 0;

pub(super) fn get_info(file: &OpenFile, raw_argument: u64) -> Result<(), Errno> {
    let address = UserAddress::parse(raw_argument, Errno::Fault)?;
    let output =
        Out::<framebuffer_abi::RoxyFramebufferInfoAbi>::parse(address.as_u64(), Errno::Fault)?;
    output.validate()?;

    let mut info = FbInfo {
        width: 0,
        height: 0,
        stride: 0,
        memory_length: 0,
        red: empty_channel(),
        green: empty_channel(),
        blue: empty_channel(),
    };

    file.ioctl(IoctlRequest::FbGetInfo(&mut info))
        .map_err(super::execute::map_ioctl_error)?;
    framebuffer_abi::write_info(output, info)
}

fn empty_channel() -> roxy_fb_types::FbChannel {
    roxy_fb_types::FbChannel { size: 0, shift: 0 }
}
