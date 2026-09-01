use crate::{ProcessGroupId, ProcessId, SessionId, table::PROCESS_TABLE};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SetPgidError {
    NoSuchProcess,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreateSessionError {
    NoSuchProcess,
    /// The caller is already a process group leader; POSIX forbids it from starting a new
    /// session (`EPERM`).
    AlreadyGroupLeader,
}

/// Moves `target` into the process group `pgid`.
///
/// # Errors
///
/// Returns an error when the target process does not exist.
pub fn set_pgid(target: ProcessId, pgid: ProcessGroupId) -> Result<(), SetPgidError> {
    let mut table = PROCESS_TABLE.lock();
    let process = table
        .processes
        .get_mut(&target)
        .ok_or(SetPgidError::NoSuchProcess)?;

    process.pgid = pgid;

    Ok(())
}

/// Makes the calling process a new session leader, placing it in its own process group.
///
/// # Errors
///
/// Returns an error when the calling process cannot be found in the process table, or when it
/// is already a process group leader (POSIX `EPERM`).
pub fn create_session() -> Result<ProcessGroupId, CreateSessionError> {
    let mut table = PROCESS_TABLE.lock();
    let process_id = table.current_process_id();
    let process = table
        .processes
        .get_mut(&process_id)
        .ok_or(CreateSessionError::NoSuchProcess)?;

    // POSIX: `setsid` fails with EPERM when the caller is already a process group leader,
    // otherwise it could strand the rest of its group in the old session.
    if process.pgid == ProcessGroupId::from(process_id) {
        return Err(CreateSessionError::AlreadyGroupLeader);
    }

    process.pgid = ProcessGroupId::from(process_id);
    process.session_id = Some(SessionId::from(process_id));

    Ok(process.pgid)
}
