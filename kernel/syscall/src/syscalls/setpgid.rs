use roxy_process::{ProcessGroupId, ProcessId, SetPgidError, set_pgid};

use crate::{
    SyscallResult, errno::Errno, numbers::SyscallNumber, syscall, unsupported::unsupported_argument,
};

syscall!(SyscallNumber::SetPgid, handle(process_id: i64, group_id: i64));

fn handle(process_id: i64, group_id: i64) -> SyscallResult {
    // POSIX: a pid of zero means the calling process.
    let target = if process_id == 0 {
        roxy_process::current_process_id()
    } else {
        ProcessId::new(process_id.cast_unsigned())
            .ok_or_else(|| unsupported_argument("setpgid.pid", process_id, Errno::Invalid))?
    };
    // POSIX: a pgid of zero makes `target` the leader of a new group.
    let group = if group_id == 0 {
        ProcessGroupId::from(target)
    } else {
        ProcessGroupId::new(group_id.cast_unsigned())
            .ok_or_else(|| unsupported_argument("setpgid.pgid", group_id, Errno::Invalid))?
    };

    set_pgid(target, group).map_err(map_setpgid_error)?;

    Ok(0)
}

const fn map_setpgid_error(error: SetPgidError) -> Errno {
    match error {
        SetPgidError::NoSuchProcess => Errno::NoSuchProcess,
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_test::kernel_test;

    use super::SYSCALL;
    use crate::{numbers::SyscallNumber, registry::Registry};

    kernel_test!("roxy-syscall::setpgid-registered", setpgid_registered, {
        assert!(
            Registry::new(&[SYSCALL])
                .syscalls
                .iter()
                .any(|s| s.number == SyscallNumber::SetPgid)
        );
    });
}
