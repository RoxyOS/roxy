use roxy_vfs::ResolvedPath;

use crate::table::PROCESS_TABLE;

/// Returns the current process's normalized absolute working directory.
///
/// # Panics
///
/// Panics when the current scheduled thread is not owned by a running process.
pub(super) fn current_working_directory() -> ResolvedPath {
    let table = PROCESS_TABLE.lock();
    let process_id = table.current_process_id();

    table.processes[&process_id].working_directory.clone()
}
