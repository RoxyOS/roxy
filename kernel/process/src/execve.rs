//! Implements successful `execve` image replacement for the current process.
//!
//! The replacement preserves process identity, the current thread, and the descriptor table. It
//! publishes and activates the new process image only after image construction succeeds.

use alloc::vec::Vec;

use roxy_arch::{Architecture, CurrentArchitectureBackend};
use roxy_memory::UserAddress;
use roxy_thread::scheduler;

use crate::{ProcessError, image, table::PROCESS_TABLE};

/// Replaces the current process image while preserving its process, thread, and descriptor state.
///
/// # Errors
///
/// Returns before changing the current image when loading or stack construction fails.
pub fn execve_current(
    path: &[u8],
    argv: &[Vec<u8>],
    envp: &[Vec<u8>],
) -> Result<(UserAddress, UserAddress), ProcessError> {
    let image = image::build(path, argv, envp)?;
    let entry = image.entry;
    let stack_pointer = image.stack_pointer;

    CurrentArchitectureBackend::without_interrupts(|| {
        let thread_id = scheduler::current_thread_id();
        let _previous = PROCESS_TABLE
            .lock()
            .replace_addrspace(thread_id, image.addrspace.clone());
        PROCESS_TABLE.lock().clear_signal_actions(thread_id);

        image.addrspace.activate();
    });

    Ok((entry, stack_pointer))
}
