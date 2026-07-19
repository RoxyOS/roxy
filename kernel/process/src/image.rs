//! Builds an unpublished process image shared by spawn and `execve`.

use alloc::vec::Vec;

use roxy_elf::{ElfError, LoadType, LoadedElf};
use roxy_memory::UserAddress;
use roxy_vfs::VfsError;
use roxy_vm::{AddrSpace, AddrSpaceHandle, VmError};

use crate::{
    ProcessError,
    startup_stack::{self, StartupStackData},
};

const INTERPRETER_BASE: u64 = 0x0000_2000_0000_0000;

pub(super) struct ProcessImage {
    pub(super) addrspace: AddrSpaceHandle,
    pub(super) entry: UserAddress,
    pub(super) stack_pointer: UserAddress,
}

struct EntryPoint {
    address: UserAddress,
    interpreter_base: u64,
}

pub(super) fn build(
    path: &[u8],
    argv: &[Vec<u8>],
    envp: &[Vec<u8>],
) -> Result<ProcessImage, ProcessError> {
    let mut addrspace = AddrSpace::new().map_err(process_vm_error)?;
    let executable = load(&mut addrspace, path, LoadType::Executable)?;
    let entry = entry_point(&mut addrspace, &executable)?;
    let stack = addrspace.map_stack().map_err(process_vm_error)?;
    let stack_pointer = startup_stack::build(
        &mut addrspace,
        stack,
        &StartupStackData {
            path,
            argv,
            envp,
            executable: &executable,
            interpreter_base: entry.interpreter_base,
        },
    )?;

    Ok(ProcessImage {
        addrspace: addrspace.into_handle(),
        entry: entry.address,
        stack_pointer,
    })
}

fn entry_point(
    addrspace: &mut AddrSpace,
    executable: &LoadedElf,
) -> Result<EntryPoint, ProcessError> {
    let Some(path) = &executable.interpreter else {
        return Ok(EntryPoint {
            address: executable.entry,
            interpreter_base: 0,
        });
    };
    let base = UserAddress::new(INTERPRETER_BASE).ok_or(ProcessError::InvalidAddressSpace)?;
    let interpreter = load(addrspace, path, LoadType::Interpreter { base })?;

    Ok(EntryPoint {
        address: interpreter.entry,
        interpreter_base: interpreter.base,
    })
}

fn load(
    addrspace: &mut AddrSpace,
    path: &[u8],
    load_type: LoadType,
) -> Result<LoadedElf, ProcessError> {
    let bytes = roxy_vfs::read(path).map_err(map_vfs_error)?;

    roxy_elf::load(addrspace, &bytes, load_type).map_err(map_elf_error)
}

fn map_vfs_error(error: VfsError) -> ProcessError {
    match error {
        VfsError::NotFound => ProcessError::FileNotFound,
        VfsError::Unsupported => ProcessError::UnsupportedFile,
        _ => ProcessError::InvalidElf,
    }
}

fn map_elf_error(error: ElfError) -> ProcessError {
    match error {
        ElfError::OutOfMemory => ProcessError::OutOfMemory,
        ElfError::UnsupportedFormat => ProcessError::UnsupportedElf,
        _ => ProcessError::InvalidElf,
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
