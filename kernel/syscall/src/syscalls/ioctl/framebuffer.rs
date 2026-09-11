use roxy_fb_types::FbInfo;
use roxy_fd::{IoctlRequest, OpenFile};
use roxy_memory::UserAddress;

use super::{framebuffer_abi, numbers};
use crate::{args::Out, args::SyscallArg, errno::Errno};

/// `ROXY_FRAMEBUFFER_GET_INFO`: report the framebuffer layout and pixel format.
///
/// The layout is fixed by the boot loader, so there is no mode-setting request: a client that
/// wants to check whether a mode is usable compares it against what this request reported. Pixel
/// access happens through `mmap` with offset zero, never through ioctl.
pub(super) const ROXY_FRAMEBUFFER_GET_INFO: u64 = numbers::FRAMEBUFFER_BASE;

/// `ROXY_FRAMEBUFFER_TAKE_CONTROL`: take exclusive control of the visible frame.
///
/// The argument is ignored. Taking suspends the framebuffer terminal's drawing so the kernel
/// console cannot paint over the client's pixels; taking a frame another process holds fails with
/// `EBUSY`, while the holder may repeat the request.
pub(super) const ROXY_FRAMEBUFFER_TAKE_CONTROL: u64 = numbers::FRAMEBUFFER_BASE + 1;

/// `ROXY_FRAMEBUFFER_RELEASE_CONTROL`: release control taken by `TAKE_CONTROL`.
///
/// The argument is ignored. Releasing resumes the framebuffer terminal on a cleared screen; a
/// process that does not hold the frame gets `EINVAL`, because releasing is not a way to seize or
/// to probe ownership.
pub(super) const ROXY_FRAMEBUFFER_RELEASE_CONTROL: u64 = numbers::FRAMEBUFFER_BASE + 2;

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

pub(super) fn take_control(file: &OpenFile) -> Result<(), Errno> {
    file.ioctl(IoctlRequest::FbTakeControl)
        .map_err(super::execute::map_ioctl_error)
}

pub(super) fn release_control(file: &OpenFile) -> Result<(), Errno> {
    file.ioctl(IoctlRequest::FbReleaseControl)
        .map_err(super::execute::map_ioctl_error)
}
