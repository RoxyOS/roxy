use core::mem::{align_of, offset_of, size_of};

use crate::{SyscallResult, args::Out, numbers::SyscallNumber, syscall};

const FIELD_SIZE: usize = 65;

syscall!(SyscallNumber::Uname, handle(output: Out<UtsnameAbi> => Fault));

/// Fixed-layout uname payload copied across the userspace syscall ABI.
#[repr(C)]
struct UtsnameAbi {
    sysname: [u8; FIELD_SIZE],
    nodename: [u8; FIELD_SIZE],
    release: [u8; FIELD_SIZE],
    version: [u8; FIELD_SIZE],
    machine: [u8; FIELD_SIZE],
    domainname: [u8; FIELD_SIZE],
}

const _: () = assert!(size_of::<UtsnameAbi>() == FIELD_SIZE * 6);
const _: () = assert!(align_of::<UtsnameAbi>() == 1);
const _: () = assert!(offset_of!(UtsnameAbi, sysname) == 0);
const _: () = assert!(offset_of!(UtsnameAbi, nodename) == FIELD_SIZE);
const _: () = assert!(offset_of!(UtsnameAbi, release) == FIELD_SIZE * 2);
const _: () = assert!(offset_of!(UtsnameAbi, version) == FIELD_SIZE * 3);
const _: () = assert!(offset_of!(UtsnameAbi, machine) == FIELD_SIZE * 4);
const _: () = assert!(offset_of!(UtsnameAbi, domainname) == FIELD_SIZE * 5);

fn handle(output: Out<UtsnameAbi>) -> SyscallResult {
    let result = UtsnameAbi::roxy();

    // SAFETY: UtsnameAbi's checked C layout contains only fully initialized byte arrays.
    unsafe { output.write(&result) }?;

    Ok(0)
}

impl UtsnameAbi {
    fn roxy() -> Self {
        Self {
            sysname: field(b"Roxy"),
            nodename: field(b"roxy"),
            release: field(b"0.1.0"),
            version: field(b"0.1.0"),
            machine: field(b"x86_64"),
            domainname: field(b"(none)"),
        }
    }
}

fn field(value: &[u8]) -> [u8; FIELD_SIZE] {
    let mut field = [0; FIELD_SIZE];
    field[..value.len()].copy_from_slice(value);

    field
}
