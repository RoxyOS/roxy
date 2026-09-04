use roxy_fd::{IoctlRequest, OpenFile};
use roxy_memory::UserAddress;

use crate::{
    args::{Out, SyscallArg, user_memory},
    errno::Errno,
};

/// pty ioctl request numbers and semantics follow the Linux `TIOC*` values under the `T`
/// direction/size layout, matching mlibc's `sysdeps/roxy` abi-bits.
/// `TIOCGPTN`: returns the allocated slave number for a pty master.
pub(super) const TIOCGPTN: u64 = 0x8004_5430;
/// `TIOCSPTLCK`: takes an `int`; non-zero locks the slave, zero unlocks it.
pub(super) const TIOCSPTLCK: u64 = 0x4004_5431;

pub(super) fn get_pty_number(file: &OpenFile, raw_argument: u64) -> Result<(), Errno> {
    let address = UserAddress::parse(raw_argument, Errno::Fault)?;
    let output = Out::<u32>::parse(address.as_u64(), Errno::Fault)?;
    output.validate()?;
    let mut number = 0u32;

    file.ioctl(IoctlRequest::PtyGetNumber(&mut number))
        .map_err(super::execute::map_ioctl_error)?;

    // SAFETY: u32 has no padding and `number` is initialized.
    unsafe { output.write(&number) }?;

    Ok(())
}

pub(super) fn set_pty_lock(file: &OpenFile, raw_argument: u64) -> Result<(), Errno> {
    let address = UserAddress::parse(raw_argument, Errno::Fault)?;
    let mut locked = 0i32;
    // SAFETY: i32 has no padding and every bit pattern is valid.
    unsafe { user_memory::read(address, &mut locked) }?;

    file.ioctl(IoctlRequest::PtySetLock(locked != 0))
        .map_err(super::execute::map_ioctl_error)
}
