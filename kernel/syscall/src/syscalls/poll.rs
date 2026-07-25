use alloc::vec;
use core::{mem, slice};

use bitflags::bitflags;
use roxy_fd::{Fd, FileError, PollEvents};
use roxy_memory::UserAddress;

use crate::{
    Syscall, SyscallResult, errno::Errno, numbers::SyscallNumber, unsupported::unsupported_argument,
};

pub(super) const SYSCALL: Syscall = Syscall::new(SyscallNumber::Poll, handle);

bitflags! {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct PollEventFlags: i16 {
        const IN = 0x0001;
        const PRI = 0x0002;
        const OUT = 0x0004;
        const ERR = 0x0008;
        const HUP = 0x0010;
        const NVAL = 0x0020;
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct PollFdAbi {
    fd: i32,
    events: i16,
    revents: i16,
}

const _: () = assert!(mem::size_of::<PollFdAbi>() == 8);

fn handle(arguments: [u64; 6]) -> SyscallResult {
    let count = usize::try_from(arguments[1]).map_err(|_| Errno::Invalid)?;
    let timeout = arguments[2].cast_signed();

    if timeout != 0 {
        return Err(unsupported_argument(
            "poll.timeout",
            timeout,
            Errno::NotSupported,
        ));
    }

    if count == 0 {
        return Ok(0);
    }

    let address = UserAddress::new(arguments[0]).ok_or(Errno::Fault)?;

    let bytes = count
        .checked_mul(mem::size_of::<PollFdAbi>())
        .ok_or(Errno::Invalid)?;
    let addrspace = roxy_process::current_addrspace().map_err(|_| Errno::Fault)?;

    addrspace
        .validate_writable(address, bytes)
        .map_err(|_| Errno::Fault)?;

    let mut entries = vec![PollFdAbi::default(); count];
    read_entries(&addrspace, address, &mut entries)?;

    let mut ready = 0;

    for entry in &mut entries {
        entry.revents = poll_entry(*entry);
        if entry.revents != 0 {
            ready += 1;
        }
    }

    write_entries(&addrspace, address, &entries)?;
    Ok(ready)
}

fn read_entries(
    addrspace: &roxy_vm::AddrSpaceHandle,
    address: UserAddress,
    entries: &mut [PollFdAbi],
) -> Result<(), Errno> {
    // SAFETY: PollFdAbi is repr(C), contains no padding, and entries is a valid writable slice.
    let bytes = unsafe {
        slice::from_raw_parts_mut(entries.as_mut_ptr().cast::<u8>(), mem::size_of_val(entries))
    };

    addrspace
        .read_bytes(address, bytes)
        .map_err(|_| Errno::Fault)
}

fn write_entries(
    addrspace: &roxy_vm::AddrSpaceHandle,
    address: UserAddress,
    entries: &[PollFdAbi],
) -> Result<(), Errno> {
    // SAFETY: PollFdAbi is repr(C), contains no padding, and entries remains borrowed here.
    let bytes =
        unsafe { slice::from_raw_parts(entries.as_ptr().cast::<u8>(), mem::size_of_val(entries)) };

    addrspace
        .write_bytes(address, bytes)
        .map_err(|_| Errno::Fault)
}

fn poll_entry(entry: PollFdAbi) -> i16 {
    if entry.fd < 0 {
        return 0;
    }

    let requested = PollEventFlags::from_bits_truncate(entry.events);
    let fd = Fd::new(entry.fd.cast_unsigned());
    let Ok(file) = roxy_process::current_open_file(fd) else {
        return PollEventFlags::NVAL.bits();
    };

    match file.poll() {
        Ok(events) => encode_events(requested, events),
        Err(FileError::BadOperation) => 0,
        Err(FileError::Io) => PollEventFlags::ERR.bits(),
    }
}

fn encode_events(requested: PollEventFlags, events: PollEvents) -> i16 {
    let mut output = PollEventFlags::empty();

    if requested.contains(PollEventFlags::IN) && events.readable {
        output.insert(PollEventFlags::IN);
    }

    if requested.contains(PollEventFlags::PRI) && events.priority {
        output.insert(PollEventFlags::PRI);
    }

    if requested.contains(PollEventFlags::OUT) && events.writable {
        output.insert(PollEventFlags::OUT);
    }

    if events.error {
        output.insert(PollEventFlags::ERR);
    }

    if events.hangup {
        output.insert(PollEventFlags::HUP);
    }

    output.bits()
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_test::kernel_test;

    use super::{PollEventFlags, encode_events};
    use roxy_fd::PollEvents;

    kernel_test!(
        "roxy-syscall::poll-event-codec",
        converts_supported_event_bits,
        {
            let reported = encode_events(
                PollEventFlags::IN | PollEventFlags::OUT,
                PollEvents {
                    readable: true,
                    writable: true,
                    hangup: true,
                    ..PollEvents::default()
                },
            );
            assert_eq!(
                reported,
                (PollEventFlags::IN | PollEventFlags::OUT | PollEventFlags::HUP).bits()
            );
        }
    );
}
