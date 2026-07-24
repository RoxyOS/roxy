use roxy_vfs::ResolvedPath;

use crate::table::{PROCESS_TABLE, ProcessTable};

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

/// Replaces the current process's normalized absolute working directory.
///
/// # Panics
///
/// Panics when the current scheduled thread is not owned by a running process.
pub fn set_current_working_directory(path: ResolvedPath) {
    let mut table = PROCESS_TABLE.lock();
    let process_id = table.current_process_id();

    table.set_working_directory(process_id, path);
}

impl ProcessTable {
    pub(super) fn set_working_directory(
        &mut self,
        process_id: crate::ProcessId,
        path: ResolvedPath,
    ) {
        self.processes
            .get_mut(&process_id)
            .unwrap()
            .working_directory = path;
    }
}
