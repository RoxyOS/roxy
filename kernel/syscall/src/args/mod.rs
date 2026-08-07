mod address;
mod c_string;
mod c_string_array;
mod fd;
mod file_permissions;
mod integer;
mod nullable;
mod out;
mod path;
mod process_id;
mod slice;
mod timespec;
pub(crate) mod user_memory;

pub(crate) use crate::syscalls::signal::SignalMask;
pub(crate) use c_string::CString;
pub(crate) use c_string_array::CStringArray;
pub(crate) use nullable::Nullable;
pub(crate) use out::Out;
pub(crate) use path::Path;
pub(crate) use slice::Slice;
pub(crate) use timespec::Timespec;

use crate::errno::Errno;

pub(crate) trait RawSyscallArg: Sized {
    fn parse(raw: u64) -> Self;
}

pub(crate) trait SyscallArg: Sized {
    fn parse(raw: u64, error: Errno) -> Result<Self, Errno>;
}

macro_rules! syscall {
    ($number:expr, $handler:ident()) => {
        pub(super) const SYSCALL: $crate::Syscall = $crate::Syscall::new($number, parse);

        fn parse(_arguments: [u64; 6]) -> $crate::SyscallResult {
            $handler()
        }
    };

    ($number:expr, $handler:ident($($name:ident: $type:ty),* $(,)?)) => {
        pub(super) const SYSCALL: $crate::Syscall = $crate::Syscall::new($number, parse);

        fn parse(arguments: [u64; 6]) -> $crate::SyscallResult {
            let mut raw = arguments.into_iter();
            $(
                let $name = <$type as $crate::args::RawSyscallArg>::parse(raw.next().unwrap());
            )*

            $handler($($name),*)
        }
    };

    ($number:expr, $handler:ident($($name:ident: $type:ty => $error:ident),* $(,)?)) => {
        pub(super) const SYSCALL: $crate::Syscall = $crate::Syscall::new($number, parse);

        fn parse(arguments: [u64; 6]) -> $crate::SyscallResult {
            let mut raw = arguments.into_iter();
            $(
                let $name = <$type as $crate::args::SyscallArg>::parse(
                    raw.next().unwrap(),
                    $crate::errno::Errno::$error,
                )?;
            )*

            $handler($($name),*)
        }
    };

    ($number:expr, $handler:ident($($name:ident: $type:ty $(=> $error:ident)?),* $(,)?)) => {
        pub(super) const SYSCALL: $crate::Syscall = $crate::Syscall::new($number, parse);

        fn parse(arguments: [u64; 6]) -> $crate::SyscallResult {
            let mut raw = arguments.into_iter();
            $(
                let $name = syscall!(@parse raw.next().unwrap(), $type $(=> $error)?);
            )*

            $handler($($name),*)
        }
    };

    (@parse $raw:expr, $type:ty) => {
        <$type as $crate::args::RawSyscallArg>::parse($raw)
    };

    (@parse $raw:expr, $type:ty => $error:ident) => {
        <$type as $crate::args::SyscallArg>::parse($raw, $crate::errno::Errno::$error)?
    };
}

pub(crate) use syscall;
