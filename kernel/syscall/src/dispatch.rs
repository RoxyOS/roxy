use roxy_abi::{Errno, SyscallNumber};
use roxy_arch::RawSyscall;

use crate::exit;

pub(super) type SyscallResult = Result<u64, Errno>;
type SyscallHandler = fn([u64; 6]) -> SyscallResult;

const SYSCALLS: [Syscall; 1] = [exit::SYSCALL];
const REGISTRY: Registry = Registry::new(&SYSCALLS);

pub(super) struct Syscall {
    number: SyscallNumber,
    handler: SyscallHandler,
}

struct Registry {
    syscalls: &'static [Syscall],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RegistryError {
    DuplicateNumber(SyscallNumber),
}

impl Syscall {
    pub(super) const fn new(number: SyscallNumber, handler: SyscallHandler) -> Self {
        Self { number, handler }
    }
}

impl Registry {
    const fn new(syscalls: &'static [Syscall]) -> Self {
        Self { syscalls }
    }

    fn validate(&self) -> Result<(), RegistryError> {
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

    fn find(&self, number: SyscallNumber) -> Option<&Syscall> {
        self.syscalls
            .iter()
            .find(|syscall| syscall.number == number)
    }

    fn dispatch(&self, request: RawSyscall) -> SyscallResult {
        let number = SyscallNumber::try_from(request.number).map_err(|()| Errno::NoSys)?;
        let syscall = self.find(number).ok_or(Errno::NoSys)?;
        (syscall.handler)(request.arguments)
    }
}

pub(super) fn validate_registry() {
    assert_eq!(REGISTRY.validate(), Ok(()), "invalid syscall registry");
}

pub(super) fn dispatch(request: RawSyscall) -> u64 {
    match REGISTRY.dispatch(request) {
        Ok(value) => value,
        Err(error) => error.encode(),
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_abi::{Errno, SyscallNumber};
    use roxy_arch::RawSyscall;
    use roxy_test::kernel_test;

    use super::{REGISTRY, Registry, RegistryError, Syscall, dispatch};

    const DUPLICATES: [Syscall; 2] = [
        Syscall::new(SyscallNumber::Exit, return_first_argument),
        Syscall::new(SyscallNumber::Exit, return_first_argument),
    ];
    const RETURNING: [Syscall; 1] = [Syscall::new(SyscallNumber::Exit, return_first_argument)];

    kernel_test!("roxy-syscall::exit-registered", exit_registered, {
        assert!(REGISTRY.find(SyscallNumber::Exit).is_some());
    });

    kernel_test!("roxy-syscall::return-value", return_value, {
        let registry = Registry::new(&RETURNING);
        let result = registry.dispatch(RawSyscall {
            number: SyscallNumber::Exit as u64,
            arguments: [42, 0, 0, 0, 0, 0],
        });

        assert_eq!(result, Ok(42));
    });

    kernel_test!("roxy-syscall::unknown-number", unknown_number, {
        let result = dispatch(RawSyscall {
            number: u64::MAX,
            arguments: [0; 6],
        });

        assert_eq!(result, Errno::NoSys.encode());
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
