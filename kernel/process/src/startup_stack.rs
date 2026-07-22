use alloc::vec::Vec;
use core::mem::size_of;

use roxy_elf::LoadedElf;
use roxy_memory::{PAGE_SIZE, UserAddress};
use roxy_vm::{AddrSpace, UserStack};

use crate::{ProcessError, image::process_vm_error};

const AT_PHDR: u64 = 3;
const AT_PHENT: u64 = 4;
const AT_PHNUM: u64 = 5;
const AT_PAGESZ: u64 = 6;
const AT_BASE: u64 = 7;
const AT_ENTRY: u64 = 9;
const AT_SECURE: u64 = 23;
const AT_EXECFN: u64 = 31;
const AUX_ENTRIES: usize = 9;

pub(super) struct StartupStackData<'a> {
    pub(super) path: &'a [u8],
    pub(super) argv: &'a [Vec<u8>],
    pub(super) envp: &'a [Vec<u8>],
    pub(super) executable: &'a LoadedElf,
    pub(super) interpreter_base: u64,
}

pub(super) fn build(
    addrspace: &mut AddrSpace,
    stack: UserStack,
    stack_data: &StartupStackData<'_>,
) -> Result<UserAddress, ProcessError> {
    let header_size = header_size(stack_data.argv.len(), stack_data.envp.len())?;
    let data_size = header_size
        .checked_add(strings_size(
            stack_data.path,
            stack_data.argv,
            stack_data.envp,
        )?)
        .ok_or(ProcessError::ArgumentsTooLarge)?;
    let stack_pointer = stack_pointer(stack, data_size)?;
    let strings_address = stack_pointer
        .checked_add(u64::try_from(header_size).unwrap())
        .ok_or(ProcessError::InvalidAddressSpace)?;
    let data = encode(stack_data, data_size, strings_address)?;

    addrspace
        .write_bytes(stack_pointer, &data)
        .map_err(process_vm_error)?;

    Ok(stack_pointer)
}

fn encode(
    stack_data: &StartupStackData<'_>,
    data_size: usize,
    strings_address: UserAddress,
) -> Result<Vec<u8>, ProcessError> {
    let mut data = Vec::new();
    data.try_reserve_exact(data_size)
        .map_err(|_| ProcessError::OutOfMemory)?;
    let mut next_string = strings_address
        .as_u64()
        .checked_add(u64::try_from(stack_data.path.len() + 1).unwrap())
        .ok_or(ProcessError::InvalidAddressSpace)?;

    push_word(&mut data, u64::try_from(stack_data.argv.len()).unwrap());
    push_string_pointers(&mut data, stack_data.argv, &mut next_string)?;
    push_word(&mut data, 0);
    push_string_pointers(&mut data, stack_data.envp, &mut next_string)?;
    push_word(&mut data, 0);
    push_auxiliary(
        &mut data,
        strings_address,
        stack_data.executable,
        stack_data.interpreter_base,
    );
    append_strings(&mut data, stack_data.path, stack_data.argv, stack_data.envp);

    debug_assert_eq!(data.len(), data_size);

    Ok(data)
}

fn push_string_pointers(
    data: &mut Vec<u8>,
    strings: &[Vec<u8>],
    next_address: &mut u64,
) -> Result<(), ProcessError> {
    for string in strings {
        push_word(data, *next_address);
        *next_address = next_address
            .checked_add(u64::try_from(string.len() + 1).unwrap())
            .ok_or(ProcessError::InvalidAddressSpace)?;
    }

    Ok(())
}

fn push_auxiliary(
    data: &mut Vec<u8>,
    execfn: UserAddress,
    executable: &LoadedElf,
    interpreter_base: u64,
) {
    let headers = &executable.program_headers;
    let entries = [
        (AT_PHDR, headers.address.as_u64()),
        (AT_PHENT, u64::from(headers.entry_size)),
        (AT_PHNUM, u64::from(headers.count)),
        (AT_ENTRY, executable.entry.as_u64()),
        (AT_BASE, interpreter_base),
        (AT_PAGESZ, PAGE_SIZE),
        (AT_EXECFN, execfn.as_u64()),
        (AT_SECURE, 0),
        (0, 0),
    ];

    for (key, value) in entries {
        push_word(data, key);
        push_word(data, value);
    }
}

fn append_strings(data: &mut Vec<u8>, path: &[u8], argv: &[Vec<u8>], envp: &[Vec<u8>]) {
    append_string(data, path);

    for string in argv.iter().chain(envp) {
        append_string(data, string);
    }
}

fn header_size(argv_count: usize, envp_count: usize) -> Result<usize, ProcessError> {
    let words = 3_usize
        .checked_add(argv_count)
        .and_then(|count| count.checked_add(envp_count))
        .and_then(|count| count.checked_add(AUX_ENTRIES * 2))
        .ok_or(ProcessError::ArgumentsTooLarge)?;

    words
        .checked_mul(size_of::<u64>())
        .ok_or(ProcessError::ArgumentsTooLarge)
}

fn strings_size(path: &[u8], argv: &[Vec<u8>], envp: &[Vec<u8>]) -> Result<usize, ProcessError> {
    argv.iter()
        .chain(envp)
        .try_fold(path.len() + 1, |size, string| {
            size.checked_add(string.len() + 1)
        })
        .ok_or(ProcessError::ArgumentsTooLarge)
}

fn stack_pointer(stack: UserStack, data_size: usize) -> Result<UserAddress, ProcessError> {
    let aligned_size = data_size
        .checked_add(15)
        .map(|size| size / 16 * 16)
        .ok_or(ProcessError::ArgumentsTooLarge)?;
    let available = usize::try_from(stack.top.as_u64() - stack.bottom.as_u64()).unwrap();

    if aligned_size > available {
        return Err(ProcessError::ArgumentsTooLarge);
    }

    UserAddress::new(stack.top.as_u64() - u64::try_from(aligned_size).unwrap())
        .ok_or(ProcessError::InvalidAddressSpace)
}

fn append_string(data: &mut Vec<u8>, value: &[u8]) {
    data.extend_from_slice(value);
    data.push(0);
}

fn push_word(data: &mut Vec<u8>, value: u64) {
    data.extend_from_slice(&value.to_ne_bytes());
}
