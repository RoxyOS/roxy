use core::sync::atomic::{AtomicU64, Ordering};

use roxy_memory::UserAddress;
use roxy_thread::{Thread, ThreadCreateError, ThreadId, scheduler};
use roxy_vm::{AddrSpace, AddrSpaceHandle, VmError};

use crate::{Process, ProcessError, ProcessId, ProcessState, startup_stack, table::PROCESS_TABLE};

static NEXT_PROCESS_ID: AtomicU64 = AtomicU64::new(1);
const INTERPRETER_BASE: u64 = 0x0000_2000_0000_0000;

struct EntryPoint {
    address: UserAddress,
    interpreter_base: u64,
}

/// Creates a single-thread process from a VFS executable and makes it runnable.
///
/// # Errors
///
/// Returns an error for an invalid ELF image, address-space failure, or allocation failure.
pub fn spawn(path: impl AsRef<[u8]>) -> Result<ProcessId, ProcessError> {
    let path = path.as_ref();
    let mut addrspace = AddrSpace::new().map_err(process_vm_error)?;
    let loaded = load_executable(&mut addrspace, path)?;
    let entry = entry_point(&mut addrspace, &loaded)?;

    let mapped_stack = addrspace.map_stack().map_err(process_vm_error)?;
    let initial_stack_pointer = startup_stack::build(
        &mut addrspace,
        mapped_stack,
        path,
        &loaded,
        entry.interpreter_base,
    )?;
    let main_thread =
        Thread::new_user(entry.address, initial_stack_pointer).map_err(thread_error)?;
    let addrspace = addrspace.into_handle();
    let process = Process::new(main_thread.id(), addrspace.clone());
    let process_id = process.id;

    PROCESS_TABLE.lock().insert(process);
    scheduler::enqueue_user(main_thread, addrspace);

    Ok(process_id)
}

fn load_executable(
    addrspace: &mut AddrSpace,
    path: &[u8],
) -> Result<roxy_elf::LoadedElf, ProcessError> {
    let image = roxy_vfs::read(path).map_err(|_| ProcessError::InvalidElf)?;

    roxy_elf::load(addrspace, &image, roxy_elf::LoadType::Executable).map_err(map_elf_error)
}

/// Selects the initial userspace entry point.
///
/// A static executable starts at its own entry point. A dynamic executable instead loads its
/// `PT_INTERP` image and starts at the interpreter entry point; the executable entry remains
/// available to the interpreter through the startup auxiliary vector.
fn entry_point(
    addrspace: &mut AddrSpace,
    executable: &roxy_elf::LoadedElf,
) -> Result<EntryPoint, ProcessError> {
    let Some(path) = &executable.interpreter else {
        return Ok(EntryPoint {
            address: executable.entry,
            interpreter_base: 0,
        });
    };
    let image = roxy_vfs::read(path).map_err(|_| ProcessError::InvalidElf)?;
    let base = UserAddress::new(INTERPRETER_BASE).ok_or(ProcessError::InvalidAddressSpace)?;
    let interpreter = roxy_elf::load(addrspace, &image, roxy_elf::LoadType::Interpreter { base })
        .map_err(map_elf_error)?;

    Ok(EntryPoint {
        address: interpreter.entry,
        interpreter_base: interpreter.base,
    })
}

fn map_elf_error(error: roxy_elf::ElfError) -> ProcessError {
    match error {
        roxy_elf::ElfError::OutOfMemory => ProcessError::OutOfMemory,
        _ => ProcessError::InvalidElf,
    }
}

impl Process {
    pub(super) fn new(main_thread_id: ThreadId, addrspace: AddrSpaceHandle) -> Self {
        Self {
            id: ProcessId(NEXT_PROCESS_ID.fetch_add(1, Ordering::Relaxed)),
            addrspace: Some(addrspace),
            main_thread_id,
            fds: roxy_fd::FdTable::new(),
            state: ProcessState::Running,
        }
    }

    pub(super) fn from_fork(
        main_thread_id: ThreadId,
        addrspace: AddrSpaceHandle,
        fds: roxy_fd::FdTable,
    ) -> Self {
        Self {
            id: ProcessId(NEXT_PROCESS_ID.fetch_add(1, Ordering::Relaxed)),
            addrspace: Some(addrspace),
            main_thread_id,
            fds,
            state: ProcessState::Running,
        }
    }
}

pub(super) fn process_vm_error(error: VmError) -> ProcessError {
    match error {
        VmError::OutOfMemory => ProcessError::OutOfMemory,
        VmError::InvalidRange
        | VmError::PartialUnmap
        | VmError::AddressInUse
        | VmError::NotMapped
        | VmError::MappingFailed
        | VmError::PermissionDenied => ProcessError::InvalidAddressSpace,
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

        assert_eq!(spawn([]), Err(ProcessError::InvalidElf));
        assert_eq!(statistics().allocated_frames, baseline);
    });
}
