use alloc::vec::Vec;
use core::mem::size_of;

use roxy_arch::{Architecture, CurrentArchitectureBackend};
use roxy_memory::{PAGE_SIZE, UserAddress};
use roxy_process::ProcessError;
use roxy_vm::AddrSpaceHandle;

use crate::{
    Syscall, SyscallResult, errno::Errno, numbers::SyscallNumber, unsupported::unsupported_argument,
};

pub(super) const SYSCALL: Syscall = Syscall::new(SyscallNumber::Execve, handle);

const STACK_BYTES: usize = 64 * 1024;
const PAGE_BYTES: usize = 4096;
const FIXED_STACK_WORDS: usize = 21;
const MAX_POINTERS: usize = STACK_BYTES / size_of::<u64>();

#[derive(Debug)]
struct ExecveRequest {
    path: Vec<u8>,
    argv: Vec<Vec<u8>>,
    envp: Vec<Vec<u8>>,
}

impl ExecveRequest {
    fn parse(addrspace: &AddrSpaceHandle, arguments: [u64; 6]) -> Result<Self, Errno> {
        let path_address = UserAddress::new(arguments[0]).ok_or(Errno::Fault)?;
        let path = read_string(addrspace, path_address, STACK_BYTES)?;

        if path.is_empty() {
            return Err(Errno::NotFound);
        }

        let argv_pointers = read_pointers(addrspace, arguments[1])?;
        let envp_pointers = read_pointers(addrspace, arguments[2])?;

        let pointer_count = argv_pointers
            .len()
            .checked_add(envp_pointers.len())
            .ok_or(Errno::TooBig)?;
        let header_size = FIXED_STACK_WORDS
            .checked_add(pointer_count)
            .and_then(|words| words.checked_mul(size_of::<u64>()))
            .ok_or(Errno::TooBig)?;
        let mut remaining = STACK_BYTES
            .checked_sub(header_size)
            .and_then(|bytes| bytes.checked_sub(path.len() + 1))
            .ok_or(Errno::TooBig)?;

        let argv = read_strings(addrspace, &argv_pointers, &mut remaining)?;
        let envp = read_strings(addrspace, &envp_pointers, &mut remaining)?;

        Ok(Self { path, argv, envp })
    }
}

fn handle(arguments: [u64; 6]) -> SyscallResult {
    let addrspace = roxy_process::current_addrspace().map_err(|_| Errno::Fault)?;
    let request = ExecveRequest::parse(&addrspace, arguments)?;

    let (entry, stack_pointer) =
        roxy_process::execve_current(&request.path, &request.argv, &request.envp)
            .map_err(map_process_error)?;

    // SAFETY: execve_current activated the fully built image containing both user addresses.
    unsafe { CurrentArchitectureBackend::resume_user(entry.as_u64(), stack_pointer.as_u64()) }
}

fn read_pointers(addrspace: &AddrSpaceHandle, raw: u64) -> Result<Vec<u64>, Errno> {
    if raw == 0 {
        return Ok(Vec::new());
    }

    let base = UserAddress::new(raw).ok_or(Errno::Fault)?;
    let mut pointers = Vec::new();

    for index in 0..=MAX_POINTERS {
        let offset = index.checked_mul(size_of::<u64>()).unwrap();
        let address = base
            .checked_add(u64::try_from(offset).unwrap())
            .ok_or(Errno::Fault)?;
        let mut bytes = [0; size_of::<u64>()];

        addrspace
            .read_bytes(address, &mut bytes)
            .map_err(|_| Errno::Fault)?;

        let pointer = u64::from_ne_bytes(bytes);

        if pointer == 0 {
            return Ok(pointers);
        }

        if index == MAX_POINTERS {
            return Err(Errno::Fault);
        }

        pointers.try_reserve(1).map_err(|_| Errno::NoMem)?;
        pointers.push(pointer);
    }

    Err(Errno::Fault)
}

fn read_strings(
    addrspace: &AddrSpaceHandle,
    pointers: &[u64],
    remaining: &mut usize,
) -> Result<Vec<Vec<u8>>, Errno> {
    let mut strings = Vec::new();

    strings
        .try_reserve_exact(pointers.len())
        .map_err(|_| Errno::NoMem)?;

    for pointer in pointers {
        let address = UserAddress::new(*pointer).ok_or(Errno::Fault)?;
        let string = read_string(addrspace, address, *remaining)?;

        *remaining = remaining
            .checked_sub(string.len() + 1)
            .ok_or(Errno::TooBig)?;
        strings.push(string);
    }

    Ok(strings)
}

fn read_string(
    addrspace: &AddrSpaceHandle,
    address: UserAddress,
    max_bytes: usize,
) -> Result<Vec<u8>, Errno> {
    let mut output = Vec::new();

    while output.len() < max_bytes {
        let current = address
            .checked_add(u64::try_from(output.len()).map_err(|_| Errno::Fault)?)
            .ok_or(Errno::Fault)?;
        let page_remaining = usize::try_from(PAGE_SIZE - current.as_u64() % PAGE_SIZE).unwrap();
        let length = page_remaining.min(max_bytes - output.len());
        let mut chunk = [0; PAGE_BYTES];

        addrspace
            .read_bytes(current, &mut chunk[..length])
            .map_err(|_| Errno::Fault)?;

        if let Some(terminator) = chunk[..length].iter().position(|byte| *byte == 0) {
            output
                .try_reserve_exact(terminator)
                .map_err(|_| Errno::NoMem)?;
            output.extend_from_slice(&chunk[..terminator]);

            return Ok(output);
        }

        output.try_reserve_exact(length).map_err(|_| Errno::NoMem)?;
        output.extend_from_slice(&chunk[..length]);
    }

    Err(Errno::TooBig)
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
