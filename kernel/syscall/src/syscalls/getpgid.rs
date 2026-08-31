use roxy_process::{ProcessId, process_pgid};

use crate::{
    SyscallResult, errno::Errno, numbers::SyscallNumber, syscall, unsupported::unsupported_argument,
};

syscall!(SyscallNumber::GetPgid, handle(pid: i64));

fn handle(pid: i64) -> SyscallResult {
    // POSIX: a pid of zero means the calling process.
    let target = if pid == 0 {
        roxy_process::current_process_id()
    } else {
        ProcessId::new(pid.cast_unsigned())
            .ok_or_else(|| unsupported_argument("getpgid.pid", pid, Errno::Invalid))?
    };

    let group = process_pgid(target).ok_or(Errno::NoSuchProcess)?;

    Ok(group.as_u64())
}
