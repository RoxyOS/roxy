use alloc::vec::Vec;

use roxy_memory::{PAGE_SIZE, UserAddress};
use roxy_vm::{AddrSpace, UserStack};

use crate::{ProcessError, creation::process_vm_error};

const AT_PHDR: u64 = 3;
const AT_PHENT: u64 = 4;
const AT_PHNUM: u64 = 5;
const AT_PAGESZ: u64 = 6;
const AT_BASE: u64 = 7;
const AT_ENTRY: u64 = 9;
const AT_SECURE: u64 = 23;
const AT_EXECFN: u64 = 31;
const HEADER_WORDS: usize = 4 + 2 * 9;

struct StackLayout {
    start: UserAddress,
    path_address: UserAddress,
}

pub(super) fn build(
    addrspace: &mut AddrSpace,
    stack: UserStack,
    path: &[u8],
    loaded: &roxy_elf::LoadedElf,
    interpreter_base: u64,
) -> Result<UserAddress, ProcessError> {
    let layout = layout(stack, path.len())?;
    let data = encode(layout.path_address, path, loaded, interpreter_base);

    addrspace
        .write_bytes(layout.start, &data)
        .map_err(process_vm_error)?;

    Ok(layout.start)
}

fn encode(
    path_address: UserAddress,
    path: &[u8],
    loaded: &roxy_elf::LoadedElf,
    interpreter_base: u64,
) -> Vec<u8> {
    let mut data = Vec::new();

    push_word(&mut data, 1); // argc

    push_word(&mut data, path_address.as_u64()); // argv
    push_word(&mut data, 0); // argv terminator

    push_word(&mut data, 0); // envp

    push_aux(&mut data, AT_PHDR, loaded.program_headers.address.as_u64());
    push_aux(
        &mut data,
        AT_PHENT,
        u64::from(loaded.program_headers.entry_size),
    );
    push_aux(&mut data, AT_PHNUM, u64::from(loaded.program_headers.count));
    push_aux(&mut data, AT_ENTRY, loaded.entry.as_u64());
    push_aux(&mut data, AT_BASE, interpreter_base);
    push_aux(&mut data, AT_PAGESZ, PAGE_SIZE);
    push_aux(&mut data, AT_EXECFN, path_address.as_u64());
    push_aux(&mut data, AT_SECURE, 0);
    push_aux(&mut data, 0, 0); // AT_NULL

    data.extend_from_slice(path);
    data.push(0);

    data
}

fn layout(stack: UserStack, path_length: usize) -> Result<StackLayout, ProcessError> {
    let header_size = HEADER_WORDS * core::mem::size_of::<u64>();
    let data_size = header_size
        .checked_add(path_length)
        .and_then(|size| size.checked_add(1))
        .ok_or(ProcessError::InvalidAddressSpace)?;
    let aligned_size = align_up(data_size, 16).ok_or(ProcessError::InvalidAddressSpace)?;
    let start = stack
        .top
        .as_u64()
        .checked_sub(u64::try_from(aligned_size).unwrap())
        .and_then(UserAddress::new)
        .filter(|start| *start >= stack.bottom)
        .ok_or(ProcessError::InvalidAddressSpace)?;
    let path_address = start
        .checked_add(u64::try_from(header_size).unwrap())
        .ok_or(ProcessError::InvalidAddressSpace)?;

    Ok(StackLayout {
        start,
        path_address,
    })
}

fn push_aux(data: &mut Vec<u8>, key: u64, value: u64) {
    push_word(data, key);
    push_word(data, value);
}

fn push_word(data: &mut Vec<u8>, value: u64) {
    data.extend_from_slice(&value.to_ne_bytes());
}

fn align_up(value: usize, alignment: usize) -> Option<usize> {
    value
        .checked_add(alignment - 1)
        .map(|value| value / alignment * alignment)
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_memory::UserAddress;
    use roxy_test::kernel_test;
    use roxy_vm::AddrSpace;

    use super::{HEADER_WORDS, build};

    kernel_test!("roxy-process::initial-user-stack", initial_user_stack, {
        let mut addrspace = AddrSpace::new().unwrap();
        let stack = addrspace.map_stack().unwrap();
        let executable = roxy_elf::LoadedElf {
            entry: UserAddress::new(0x40_1000).unwrap(),
            base: 0,
            program_headers: roxy_elf::ProgramHeaders {
                address: UserAddress::new(0x40_0040).unwrap(),
                entry_size: 56,
                count: 7,
            },
            interpreter: Some(b"/usr/lib/ld.so".to_vec()),
        };
        let pointer = build(
            &mut addrspace,
            stack,
            b"/bin/program",
            &executable,
            0x20_0000_0000,
        )
        .unwrap();
        let mut header = alloc::vec![0; HEADER_WORDS * 8];

        addrspace.read_bytes(pointer, &mut header).unwrap();

        assert_eq!(word(&header, 0), 1);
        assert_eq!(word(&header, 2), 0);
        assert_eq!(word(&header, 3), 0);
        assert_eq!(pointer.as_u64() % 16, 0);
        assert_eq!(word(&header, 4), 3);
        assert_eq!(word(&header, 5), 0x40_0040);
        assert_eq!(word(&header, 12), 7);
        assert_eq!(word(&header, 13), 0x20_0000_0000);
    });

    fn word(bytes: &[u8], index: usize) -> u64 {
        let offset = index * 8;

        u64::from_ne_bytes(bytes[offset..offset + 8].try_into().unwrap())
    }
}
