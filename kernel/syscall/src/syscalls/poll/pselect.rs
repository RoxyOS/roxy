use alloc::vec::Vec;
use core::mem;

use roxy_memory::UserAddress;

use crate::{
    SyscallResult,
    args::{Nullable, SignalSet, SyscallArg, Timespec, user_memory},
    errno::Errno,
    numbers::SyscallNumber,
    syscall,
};

use super::{PollEventFlags, PollFdAbi, poll_until_ready};

const FD_SET_WORDS: usize = 16;
const FD_SET_SIZE: usize = FD_SET_WORDS * 64;

syscall!(SyscallNumber::Pselect, handle(count: FdCount => Invalid, read: Nullable<FdSet> => Fault, write: Nullable<FdSet> => Fault, exception: Nullable<FdSet> => Fault, timeout: Nullable<Timespec> => Fault, signal_mask: Nullable<SignalSet> => Fault));

fn handle(
    count: FdCount,
    read: Nullable<FdSet>,
    write: Nullable<FdSet>,
    exception: Nullable<FdSet>,
    timeout: Nullable<Timespec>,
    signal_mask: Nullable<SignalSet>,
) -> SyscallResult {
    let timeout = match timeout {
        Nullable::Null => None,
        Nullable::Value(timeout) => Some(timeout.duration()),
    };

    let old_mask = match signal_mask {
        Nullable::Null => None,
        Nullable::Value(signal_mask) => Some(roxy_process::replace_masked_signals(signal_mask)),
    };

    let result = pselect(count.0, read, write, exception, timeout);

    if let Some(old_mask) = old_mask {
        roxy_process::replace_masked_signals(old_mask);
    }

    result
}

fn pselect(
    count: usize,
    mut read: Nullable<FdSet>,
    mut write: Nullable<FdSet>,
    mut exception: Nullable<FdSet>,
    timeout: Option<core::time::Duration>,
) -> SyscallResult {
    let mut entries = poll_entries(count, &read, &write, &exception);
    let ready = poll_until_ready(&mut entries, timeout)?;

    update_sets(&entries, &mut read, &mut write, &mut exception)?;

    Ok(ready as u64)
}

fn poll_entries(
    count: usize,
    read: &Nullable<FdSet>,
    write: &Nullable<FdSet>,
    exception: &Nullable<FdSet>,
) -> Vec<PollFdAbi> {
    let mut entries = Vec::new();

    for fd in 0..count {
        let mut events = PollEventFlags::empty();

        if read.contains(fd) {
            events.insert(PollEventFlags::IN);
        }

        if write.contains(fd) {
            events.insert(PollEventFlags::OUT);
        }

        if exception.contains(fd) {
            events.insert(PollEventFlags::PRI);
        }

        if !events.is_empty() {
            entries.push(PollFdAbi {
                fd: i32::try_from(fd).expect("descriptor indices are bounded by FD_SET_SIZE"),
                events: events.bits(),
                revents: 0,
            });
        }
    }

    entries
}

fn update_sets(
    entries: &[PollFdAbi],
    read: &mut Nullable<FdSet>,
    write: &mut Nullable<FdSet>,
    exception: &mut Nullable<FdSet>,
) -> Result<(), Errno> {
    read.clear();
    write.clear();
    exception.clear();

    for entry in entries {
        let events = PollEventFlags::from_bits_retain(entry.revents);
        if events.contains(PollEventFlags::NVAL) {
            return Err(Errno::BadFd);
        }

        let fd = usize::try_from(entry.fd).expect("descriptor indices are bounded by FD_SET_SIZE");
        read.set(fd, events.contains(PollEventFlags::IN));
        write.set(fd, events.contains(PollEventFlags::OUT));
        exception.set(fd, events.contains(PollEventFlags::PRI));
    }

    read.write()?;
    write.write()?;
    exception.write()
}

#[derive(Clone, Copy)]
struct FdCount(usize);

impl SyscallArg for FdCount {
    fn parse(raw: u64, error: Errno) -> Result<Self, Errno> {
        let count = usize::try_from(raw).map_err(|_| error)?;

        if count > FD_SET_SIZE {
            return Err(error);
        }

        Ok(Self(count))
    }
}

#[repr(C)]
struct FdSetAbi {
    bits: [u64; FD_SET_WORDS],
}

const _: () = assert!(mem::size_of::<FdSetAbi>() == 128);

#[derive(Clone, Copy)]
struct FdSet {
    address: UserAddress,
    bits: [u64; FD_SET_WORDS],
}

impl FdSet {
    fn contains(&self, fd: usize) -> bool {
        self.bits[fd / 64] & (1 << (fd % 64)) != 0
    }

    fn clear(&mut self) {
        self.bits.fill(0);
    }

    fn set(&mut self, fd: usize, value: bool) {
        if value {
            self.bits[fd / 64] |= 1 << (fd % 64);
        }
    }

    fn write(&self) -> Result<(), Errno> {
        let value = FdSetAbi { bits: self.bits };

        // SAFETY: FdSetAbi has a checked C layout, is fully initialized, and contains integers.
        unsafe { user_memory::write(self.address, &value) }
    }
}

impl SyscallArg for FdSet {
    fn parse(raw: u64, error: Errno) -> Result<Self, Errno> {
        let address = UserAddress::parse(raw, error)?;
        let mut value = FdSetAbi {
            bits: [0; FD_SET_WORDS],
        };

        // SAFETY: FdSetAbi has a checked C layout and contains only integer fields.
        unsafe { user_memory::read(address, &mut value) }?;

        Ok(Self {
            address,
            bits: value.bits,
        })
    }
}

impl Nullable<FdSet> {
    fn contains(&self, fd: usize) -> bool {
        match self {
            Self::Null => false,
            Self::Value(set) => set.contains(fd),
        }
    }

    fn clear(&mut self) {
        if let Self::Value(set) = self {
            set.clear();
        }
    }

    fn set(&mut self, fd: usize, value: bool) {
        if let Self::Value(set) = self {
            set.set(fd, value);
        }
    }

    fn write(&self) -> Result<(), Errno> {
        match self {
            Self::Null => Ok(()),
            Self::Value(set) => set.write(),
        }
    }
}
