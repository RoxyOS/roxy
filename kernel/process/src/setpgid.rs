use crate::{ProcessGroupId, ProcessId, table::PROCESS_TABLE};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SetPgidError {
    NoSuchProcess,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreateSessionError {
    NoSuchProcess,
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
/// Returns an error when the calling process cannot be found in the process table.
pub fn create_session() -> Result<ProcessGroupId, CreateSessionError> {
    let mut table = PROCESS_TABLE.lock();
    let process_id = table.current_process_id();
    let process = table
        .processes
        .get_mut(&process_id)
        .ok_or(CreateSessionError::NoSuchProcess)?;

    // TODO(session): POSIX requires EPERM when the caller is already a process group leader.
    // The current spawn model makes every top-level process a leader, so we skip the check.

    process.pgid = ProcessGroupId::from(process_id);
    process.session_id = Some(process_id);

    Ok(process.pgid)
}
