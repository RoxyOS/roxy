use alloc::vec::Vec;

use roxy_memory::UserAddress;
use roxy_vfs::ResolvedPath;

use crate::{Syscall, SyscallResult, errno::Errno, numbers::SyscallNumber};

pub(super) const SYSCALL: Syscall = Syscall::new(SyscallNumber::Getcwd, handle);

fn handle(arguments: [u64; 6]) -> SyscallResult {
    let output = UserAddress::new(arguments[0]).ok_or(Errno::Fault)?;
    let size = usize::try_from(arguments[1]).map_err(|_| Errno::Range)?;

    let path = roxy_process::current_working_directory();
    let encoded = encode(&path, size)?;
    let addrspace = roxy_process::current_addrspace().map_err(|_| Errno::Fault)?;

    addrspace
        .write_bytes(output, &encoded)
        .map_err(|_| Errno::Fault)?;

    Ok(u64::try_from(encoded.len()).unwrap())
}

fn encode(path: &ResolvedPath, size: usize) -> Result<Vec<u8>, Errno> {
    let required_size = path
        .as_bytes()
        .len()
        .checked_add(1)
        .ok_or(Errno::Overflow)?;

    if size < required_size {
        return Err(Errno::Range);
    }

    let mut encoded = Vec::with_capacity(required_size);
    encoded.extend_from_slice(path.as_bytes());
    encoded.push(0);

    Ok(encoded)
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_test::kernel_test;
    use roxy_vfs::ResolvedPath;

    use super::encode;
    use crate::errno::Errno;

    kernel_test!("roxy-syscall::getcwd-encoding", encodes_path, {
        let path = ResolvedPath::resolve(b"/usr/bin").unwrap();

        assert_eq!(encode(&path, 9).unwrap(), b"/usr/bin\0");
        assert_eq!(encode(&path, 8), Err(Errno::Range));
    });
}
