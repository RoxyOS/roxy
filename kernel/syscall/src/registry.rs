use crate::{Syscall, numbers::SyscallNumber, syscalls};

pub(super) const REGISTRY: Registry = Registry::new(&syscalls::SYSCALLS);

pub(super) struct Registry {
    pub(super) syscalls: &'static [Syscall],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RegistryError {
    DuplicateNumber(SyscallNumber),
}

impl Registry {
    pub(super) const fn new(syscalls: &'static [Syscall]) -> Self {
        Self { syscalls }
    }

    pub(super) fn validate(&self) -> Result<(), RegistryError> {
        for (index, syscall) in self.syscalls.iter().enumerate() {
            if self.syscalls[index + 1..]
                .iter()
                .any(|candidate| candidate.number == syscall.number)
            {
                return Err(RegistryError::DuplicateNumber(syscall.number));
            }
        }

        Ok(())
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_test::kernel_test;

    use super::{REGISTRY, Registry, RegistryError};
    use crate::{Syscall, errno::Errno, numbers::SyscallNumber};

    const DUPLICATES: [Syscall; 2] = [
        Syscall::new(SyscallNumber::Exit, return_first_argument),
        Syscall::new(SyscallNumber::Exit, return_first_argument),
    ];

    kernel_test!("roxy-syscall::required-registered", required_registered, {
        assert!(
            REGISTRY
                .syscalls
                .iter()
                .any(|syscall| syscall.number == SyscallNumber::Exit)
        );
        assert!(
            REGISTRY
                .syscalls
                .iter()
                .any(|syscall| syscall.number == SyscallNumber::FutexWait)
        );
        assert!(
            REGISTRY
                .syscalls
                .iter()
                .any(|syscall| syscall.number == SyscallNumber::FutexWake)
        );
        assert!(
            REGISTRY
                .syscalls
                .iter()
                .any(|syscall| syscall.number == SyscallNumber::AnonAllocate)
        );
        assert!(
            REGISTRY
                .syscalls
                .iter()
                .any(|syscall| syscall.number == SyscallNumber::AnonFree)
        );
        assert!(
            REGISTRY
                .syscalls
                .iter()
                .any(|syscall| syscall.number == SyscallNumber::ClockGet)
        );
        assert!(
            REGISTRY
                .syscalls
                .iter()
                .any(|syscall| syscall.number == SyscallNumber::VmMap)
        );
        assert!(
            REGISTRY
                .syscalls
                .iter()
                .any(|syscall| syscall.number == SyscallNumber::VmUnmap)
        );
        assert!(
            REGISTRY
                .syscalls
                .iter()
                .any(|syscall| syscall.number == SyscallNumber::Close)
        );
        assert!(
            REGISTRY
                .syscalls
                .iter()
                .any(|syscall| syscall.number == SyscallNumber::Seek)
        );
        assert!(
            REGISTRY
                .syscalls
                .iter()
                .any(|syscall| syscall.number == SyscallNumber::Isatty)
        );
        assert!(
            REGISTRY
                .syscalls
                .iter()
                .any(|syscall| syscall.number == SyscallNumber::Open)
        );
        assert!(
            REGISTRY
                .syscalls
                .iter()
                .any(|syscall| syscall.number == SyscallNumber::VmProtect)
        );
        assert!(
            REGISTRY
                .syscalls
                .iter()
                .any(|syscall| syscall.number == SyscallNumber::Stat)
        );
        assert!(
            REGISTRY
                .syscalls
                .iter()
                .any(|syscall| syscall.number == SyscallNumber::Fork)
        );
        assert!(
            REGISTRY
                .syscalls
                .iter()
                .any(|syscall| syscall.number == SyscallNumber::Execve)
        );
        assert!(
            REGISTRY
                .syscalls
                .iter()
                .any(|syscall| syscall.number == SyscallNumber::Getpid)
        );
        assert!(
            REGISTRY
                .syscalls
                .iter()
                .any(|syscall| syscall.number == SyscallNumber::Getppid)
        );
        assert!(
            REGISTRY
                .syscalls
                .iter()
                .any(|syscall| syscall.number == SyscallNumber::Geteuid)
        );
        assert!(
            REGISTRY
                .syscalls
                .iter()
                .any(|syscall| syscall.number == SyscallNumber::Getuid)
        );
        assert!(
            REGISTRY
                .syscalls
                .iter()
                .any(|syscall| syscall.number == SyscallNumber::Getgid)
        );
        assert!(
            REGISTRY
                .syscalls
                .iter()
                .any(|syscall| syscall.number == SyscallNumber::Getegid)
        );
        assert!(
            REGISTRY
                .syscalls
                .iter()
                .any(|syscall| syscall.number == SyscallNumber::Waitpid)
        );
        assert!(
            REGISTRY
                .syscalls
                .iter()
                .any(|syscall| syscall.number == SyscallNumber::Sigprocmask)
        );
        assert!(
            REGISTRY
                .syscalls
                .iter()
                .any(|syscall| syscall.number == SyscallNumber::Sigaction)
        );
        assert!(
            REGISTRY
                .syscalls
                .iter()
                .any(|syscall| syscall.number == SyscallNumber::OpenDir)
        );
        assert!(
            REGISTRY
                .syscalls
                .iter()
                .any(|syscall| syscall.number == SyscallNumber::ReadEntries)
        );
        assert!(
            REGISTRY
                .syscalls
                .iter()
                .any(|syscall| syscall.number == SyscallNumber::Chdir)
        );
        assert!(
            REGISTRY
                .syscalls
                .iter()
                .any(|syscall| syscall.number == SyscallNumber::Ioctl)
        );
        assert!(
            REGISTRY
                .syscalls
                .iter()
                .any(|syscall| syscall.number == SyscallNumber::Getcwd)
        );
        assert!(
            REGISTRY
                .syscalls
                .iter()
                .any(|syscall| syscall.number == SyscallNumber::SendSignal)
        );
        assert!(
            REGISTRY
                .syscalls
                .iter()
                .any(|syscall| syscall.number == SyscallNumber::Ppoll)
        );
        assert!(
            REGISTRY
                .syscalls
                .iter()
                .any(|syscall| syscall.number == SyscallNumber::Uname)
        );
        assert!(
            REGISTRY
                .syscalls
                .iter()
                .any(|syscall| syscall.number == SyscallNumber::Mkdirat)
        );
        assert!(
            REGISTRY
                .syscalls
                .iter()
                .any(|syscall| syscall.number == SyscallNumber::Unlinkat)
        );
        assert!(
            REGISTRY
                .syscalls
                .iter()
                .any(|syscall| syscall.number == SyscallNumber::Readlinkat)
        );
        assert!(
            REGISTRY
                .syscalls
                .iter()
                .any(|syscall| syscall.number == SyscallNumber::Linkat)
        );
        assert!(
            REGISTRY
                .syscalls
                .iter()
                .any(|syscall| syscall.number == SyscallNumber::Symlinkat)
        );
        assert!(
            REGISTRY
                .syscalls
                .iter()
                .any(|syscall| syscall.number == SyscallNumber::Renameat)
        );
        assert!(
            REGISTRY
                .syscalls
                .iter()
                .any(|syscall| syscall.number == SyscallNumber::Sync)
        );
        assert!(
            REGISTRY
                .syscalls
                .iter()
                .any(|syscall| syscall.number == SyscallNumber::Fsync)
        );
        assert!(
            REGISTRY
                .syscalls
                .iter()
                .any(|syscall| syscall.number == SyscallNumber::Ftruncate)
        );
        assert!(
            REGISTRY
                .syscalls
                .iter()
                .any(|syscall| syscall.number == SyscallNumber::Socketpair)
        );
        assert!(
            REGISTRY
                .syscalls
                .iter()
                .any(|syscall| syscall.number == SyscallNumber::Socket)
        );
        assert!(
            REGISTRY
                .syscalls
                .iter()
                .any(|syscall| syscall.number == SyscallNumber::Bind)
        );
        assert!(
            REGISTRY
                .syscalls
                .iter()
                .any(|syscall| syscall.number == SyscallNumber::Listen)
        );
        assert!(
            REGISTRY
                .syscalls
                .iter()
                .any(|syscall| syscall.number == SyscallNumber::Accept)
        );
        assert!(
            REGISTRY
                .syscalls
                .iter()
                .any(|syscall| syscall.number == SyscallNumber::Connect)
        );
        assert!(
            REGISTRY
                .syscalls
                .iter()
                .any(|syscall| syscall.number == SyscallNumber::Sigreturn)
        );
    });

    kernel_test!("roxy-syscall::duplicate-number", duplicate_number, {
        let registry = Registry::new(&DUPLICATES);

        assert_eq!(
            registry.validate(),
            Err(RegistryError::DuplicateNumber(SyscallNumber::Exit))
        );
    });

    fn return_first_argument(arguments: [u64; 6]) -> Result<u64, Errno> {
        (arguments[0] != u64::MAX)
            .then_some(arguments[0])
            .ok_or(Errno::NoSys)
    }
}
