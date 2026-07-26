mod address;
mod c_string;
mod c_string_array;
mod fd;
mod integer;
mod out;
mod process_id;
mod signal;
mod slice;
pub(crate) mod user_memory;

pub(crate) use c_string::CString;
pub(crate) use c_string_array::CStringArray;
pub(crate) use out::Out;
pub(crate) use slice::Slice;

use crate::errno::Errno;

pub(crate) trait RawSyscallArg: Sized {
    fn parse(raw: u64) -> Self;
}

pub(crate) trait SyscallArg: Sized {
    fn parse(raw: u64, error: Errno) -> Result<Self, Errno>;
}

macro_rules! parse_arg {
    ($raw:expr, $type:ty) => {
        <$type as $crate::args::RawSyscallArg>::parse($raw)
    };
    ($raw:expr, $type:ty => $error:ident) => {
        <$type as $crate::args::SyscallArg>::parse($raw, $crate::errno::Errno::$error)?
    };
}

macro_rules! syscall {
    ($number:expr, $handler:ident()) => {
        pub(super) const SYSCALL: $crate::Syscall = $crate::Syscall::new($number, parse);

        fn parse(_arguments: [u64; 6]) -> $crate::SyscallResult {
            $handler()
        }
    };

    ($number:expr, $handler:ident($($name:ident: $type:ty $(=> $error:ident)?),* $(,)?)) => {
        pub(super) const SYSCALL: $crate::Syscall = $crate::Syscall::new($number, parse);

        fn parse(arguments: [u64; 6]) -> $crate::SyscallResult {
            let mut raw = arguments.into_iter();
            $(
                let $name = $crate::args::parse_arg!(
                    raw.next().unwrap(),
                    $type $(=> $error)?
                );
            )*

            $handler($($name),*)
        }
    };
}

pub(crate) use {parse_arg, syscall};
