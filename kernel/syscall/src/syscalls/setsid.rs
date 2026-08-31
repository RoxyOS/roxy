use roxy_process::{CreateSessionError, create_session};

use crate::{SyscallResult, errno::Errno, numbers::SyscallNumber, syscall};

syscall!(SyscallNumber::SetSid, handle());

fn handle() -> SyscallResult {
    let sid = create_session().map_err(map_create_session_error)?;

    Ok(sid.as_u64())
}

const fn map_create_session_error(error: CreateSessionError) -> Errno {
    match error {
        CreateSessionError::NoSuchProcess => Errno::NoSuchProcess,
    }
}
