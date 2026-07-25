use alloc::vec::Vec;

use roxy_arch::{Architecture, CurrentArchitectureBackend};
use roxy_process::ProcessError;

use crate::{
    SyscallResult,
    args::{CString, CStringArray},
    errno::Errno,
    numbers::SyscallNumber,
    syscall,
    unsupported::unsupported_argument,
};

syscall!(SyscallNumber::Execve, handle(path: CString => Fault, argv: u64, envp: u64));

#[derive(Debug)]
struct ExecveRequest {
    path: Vec<u8>,
    argv: Vec<Vec<u8>>,
    envp: Vec<Vec<u8>>,
}

impl ExecveRequest {
    fn parse(path: CString, argv: u64, envp: u64) -> Result<Self, Errno> {
        if path.is_empty() {
            return Err(Errno::NotFound);
        }

        Ok(Self {
            path: path.into_inner(),
            argv: CStringArray::from_raw(argv, Errno::Fault)?.into_inner(),
            envp: CStringArray::from_raw(envp, Errno::Fault)?.into_inner(),
        })
    }
}

fn handle(path: CString, argv: u64, envp: u64) -> SyscallResult {
    let request = ExecveRequest::parse(path, argv, envp)?;

    let (entry, stack_pointer) =
        roxy_process::execve_current(&request.path, &request.argv, &request.envp)
            .map_err(map_process_error)?;

    // SAFETY: execve_current activated the fully built image containing both user addresses.
    unsafe { CurrentArchitectureBackend::resume_user(entry.as_u64(), stack_pointer.as_u64()) }
}

fn map_process_error(error: ProcessError) -> Errno {
    match error {
        ProcessError::ArgumentsTooLarge => Errno::TooBig,
        ProcessError::FileNotFound => Errno::NotFound,
        ProcessError::InvalidElf | ProcessError::InvalidAddressSpace => Errno::ExecFormat,
        ProcessError::OutOfMemory => Errno::NoMem,
        ProcessError::UnsupportedElf => {
            unsupported_argument("execve.elf_format", "unsupported", Errno::ExecFormat)
        }
        ProcessError::UnsupportedFile => {
            unsupported_argument("execve.file_read", "filesystem", Errno::ExecFormat)
        }
    }
}
