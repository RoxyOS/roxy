use roxy_arch::RawSyscall;

use crate::{
    errno::Errno,
    numbers::SyscallNumber,
    registry::{REGISTRY, Registry},
};

impl Registry {
    pub(super) fn dispatch(
        &self,
        number: SyscallNumber,
        request: RawSyscall,
    ) -> crate::SyscallResult {
        let syscall = self
            .syscalls
            .iter()
            .find(|syscall| syscall.number == number)
            .ok_or_else(|| {
                crate::unsupported::unsupported_argument("syscall", number as u64, Errno::NoSys)
            })?;

        match syscall.handler {
            crate::Handler::Arguments(handler) => handler(request.arguments),
            crate::Handler::Context(handler) => handler(request),
        }
    }
}

pub(super) fn dispatch(request: RawSyscall) -> u64 {
    let result = SyscallNumber::try_from(request.number)
        .map_err(|()| {
            crate::unsupported::unsupported_argument("syscall", request.number, Errno::NoSys)
        })
        .and_then(|number| REGISTRY.dispatch(number, request));

    let value = match result {
        Ok(value) => value,
        Err(error) => error.encode(),
    };

    roxy_process::process_latest_signal();

    value
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_arch::RawSyscall;
    use roxy_test::kernel_test;

    use super::dispatch;
    use crate::{Syscall, errno::Errno, numbers::SyscallNumber, registry::Registry};

    const RETURNING: [Syscall; 1] = [Syscall::new(SyscallNumber::Exit, return_first_argument)];

    kernel_test!("roxy-syscall::return-value", return_value, {
        let registry = Registry::new(&RETURNING);

        assert_eq!(
            registry.dispatch(
                SyscallNumber::Exit,
                RawSyscall {
                    number: SyscallNumber::Exit as u64,
                    arguments: [42, 0, 0, 0, 0, 0],
                    ..RawSyscall::default()
                },
            ),
            Ok(42)
        );
    });

    kernel_test!("roxy-syscall::unknown-number", unknown_number, {
        let result = dispatch(RawSyscall {
            number: u64::MAX,
            arguments: [0; 6],
            ..RawSyscall::default()
        });

        assert_eq!(result, Errno::NoSys.encode());
    });

    fn return_first_argument(arguments: [u64; 6]) -> Result<u64, Errno> {
        (arguments[0] != u64::MAX)
            .then_some(arguments[0])
            .ok_or(Errno::NoSys)
    }
}
