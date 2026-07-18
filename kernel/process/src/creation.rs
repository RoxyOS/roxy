use core::sync::atomic::{AtomicU64, Ordering};

use roxy_thread::{Thread, ThreadCreateError, ThreadId, scheduler};
use roxy_vm::{AddrSpace, AddrSpaceHandle, VmError};

use crate::{Process, ProcessError, ProcessId, ProcessState, table::PROCESS_TABLE};

static NEXT_PROCESS_ID: AtomicU64 = AtomicU64::new(1);

/// Creates a single-thread process from an in-memory executable image and makes it runnable.
///
/// # Errors
///
/// Returns an error for an invalid ELF image, address-space failure, or allocation failure.
pub fn spawn(image: &[u8]) -> Result<ProcessId, ProcessError> {
    let mut addrspace = AddrSpace::new().map_err(process_vm_error)?;
    let loaded = roxy_elf::load(&mut addrspace, image).map_err(|error| match error {
        roxy_elf::ElfError::OutOfMemory => ProcessError::OutOfMemory,
        _ => ProcessError::InvalidElf,
    })?;

    let stack = addrspace.map_stack().map_err(process_vm_error)?;
    let main_thread = Thread::new_user(loaded.entry, stack.top).map_err(thread_error)?;
    let addrspace = addrspace.into_handle();
    let process = Process::new(main_thread.id(), addrspace.clone());
    let process_id = process.id;

    PROCESS_TABLE.lock().insert(process);
    scheduler::enqueue_user(main_thread, addrspace);
    Ok(process_id)
}

impl Process {
    pub(super) fn new(main_thread_id: ThreadId, addrspace: AddrSpaceHandle) -> Self {
        Self {
            id: ProcessId(NEXT_PROCESS_ID.fetch_add(1, Ordering::Relaxed)),
            addrspace: Some(addrspace),
            main_thread_id,
            state: ProcessState::Running,
        }
    }
}

fn process_vm_error(error: VmError) -> ProcessError {
    match error {
        VmError::OutOfMemory => ProcessError::OutOfMemory,
        VmError::InvalidRange
        | VmError::AddressInUse
        | VmError::NotMapped
        | VmError::MappingFailed => ProcessError::InvalidAddressSpace,
    }
}

fn thread_error(error: ThreadCreateError) -> ProcessError {
    match error {
        ThreadCreateError::OutOfMemory => ProcessError::OutOfMemory,
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_memory::statistics;
    use roxy_test::kernel_test;

    use super::spawn;
    use crate::ProcessError;

    kernel_test!("roxy-process::reject-invalid-elf", reject_invalid_elf, {
        let baseline = statistics().allocated_frames;
        assert_eq!(spawn(&[]), Err(ProcessError::InvalidElf));
        assert_eq!(statistics().allocated_frames, baseline);
    });
}
