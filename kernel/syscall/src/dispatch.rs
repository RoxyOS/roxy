use roxy_abi::{Errno, SyscallNumber};
use roxy_arch::RawSyscall;

use crate::{
    SyscallResult,
    registry::{REGISTRY, Registry},
};

impl Registry {
    pub(super) fn dispatch(&self, number: SyscallNumber, arguments: [u64; 6]) -> SyscallResult {
        let syscall = self
            .syscalls
            .iter()
            .find(|syscall| syscall.number == number)
            .ok_or(Errno::NoSys)?;

        (syscall.handler)(arguments)
    }
}

pub(super) fn dispatch(request: RawSyscall) -> u64 {
    let result = SyscallNumber::try_from(request.number)
        .map_err(|()| Errno::NoSys)
        .and_then(|number| REGISTRY.dispatch(number, request.arguments));

    match result {
        Ok(value) => value,
        Err(error) => error.encode(),
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_abi::{Errno, SyscallNumber};
    use roxy_arch::RawSyscall;
    use roxy_test::kernel_test;

    use super::dispatch;
    use crate::{Syscall, registry::Registry};

    const RETURNING: [Syscall; 1] = [Syscall::new(SyscallNumber::Exit, return_first_argument)];

    kernel_test!("roxy-syscall::return-value", return_value, {
        let registry = Registry::new(&RETURNING);

        assert_eq!(
            registry.dispatch(SyscallNumber::Exit, [42, 0, 0, 0, 0, 0]),
            Ok(42)
        );
    });

    kernel_test!("roxy-syscall::unknown-number", unknown_number, {
        let result = dispatch(RawSyscall {
            number: u64::MAX,
            arguments: [0; 6],
        });

        assert_eq!(result, Errno::NoSys.encode());
    });

    fn return_first_argument(arguments: [u64; 6]) -> Result<u64, Errno> {
        (arguments[0] != u64::MAX)
            .then_some(arguments[0])
            .ok_or(Errno::NoSys)
    }
}
