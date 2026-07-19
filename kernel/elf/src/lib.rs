#![no_std]

extern crate alloc;

use alloc::vec::Vec;

use roxy_memory::UserAddress;
use roxy_vm::AddrSpace;

mod loader;
mod metadata;
mod segment;
#[cfg(feature = "kernel-test")]
mod test_utils;
mod validation;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoadType {
    Executable,
    Interpreter { base: UserAddress },
}

impl LoadType {
    pub(crate) fn bias(self) -> u64 {
        match self {
            Self::Executable => 0,
            Self::Interpreter { base } => base.as_u64(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoadedElf {
    pub entry: UserAddress,
    pub base: u64,
    pub program_headers: ProgramHeaders,
    pub interpreter: Option<Vec<u8>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProgramHeaders {
    pub address: UserAddress,
    pub entry_size: u16,
    pub count: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ElfError {
    InvalidImage,
    UnsupportedFormat,
    InvalidSegment,
    OverlappingSegments,
    WritableExecutableSegment,
    InvalidEntry,
    InvalidInterpreter,
    InvalidProgramHeaders,
    OutOfMemory,
}

/// Eagerly maps a validated ELF image into an address space.
///
/// # Errors
///
/// Returns an error for unsupported or malformed ELF input, conflicting mappings, or allocation
/// failure.
pub fn load(
    addrspace: &mut AddrSpace,
    image: &[u8],
    load_type: LoadType,
) -> Result<LoadedElf, ElfError> {
    loader::load(addrspace, image, load_type)
}
