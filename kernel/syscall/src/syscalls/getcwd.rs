use alloc::vec::Vec;

use roxy_memory::UserAddress;
use roxy_vfs::ResolvedPath;

use crate::{SyscallResult, args::Slice, errno::Errno, numbers::SyscallNumber, syscall};

syscall!(SyscallNumber::Getcwd, handle(output: UserAddress => Fault, size: usize => Range));

fn handle(output: UserAddress, size: usize) -> SyscallResult {
    let path = roxy_process::current_working_directory();
    let encoded = encode(&path, size)?;
    let output = Slice::<u8>::new(output, size);

    // SAFETY: u8 has no padding and encoded contains only initialized bytes.
    unsafe { output.write(&encoded) }?;

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
