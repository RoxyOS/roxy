use roxy_process::ProcessId;

use super::SyscallArg;
use crate::{errno::Errno, unsupported::unsupported_argument};

impl SyscallArg for ProcessId {
    fn parse(raw: u64, _error: Errno) -> Result<Self, Errno> {
        let pid = raw.cast_signed();
        let Some(process_id) = (pid > 0).then(|| ProcessId::new(pid.cast_unsigned()).unwrap())
        else {
            return Err(unsupported_argument("process_id", pid, Errno::NotSupported));
        };

        Ok(process_id)
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_process::ProcessId;
    use roxy_test::kernel_test;

    use super::SyscallArg;
    use crate::errno::Errno;

    kernel_test!("roxy-syscall::process-id-argument", validates_process_id, {
        assert_eq!(ProcessId::parse(42, Errno::Invalid).unwrap().as_u64(), 42);
        assert_eq!(
            ProcessId::parse(0, Errno::Invalid),
            Err(Errno::NotSupported)
        );
    });
}
