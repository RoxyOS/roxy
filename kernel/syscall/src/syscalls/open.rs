use core::mem::{align_of, offset_of, size_of};

use alloc::boxed::Box;

use bitflags::bitflags;
use roxy_fd::{OpenFile, StatusFlags};
use roxy_memory::UserAddress;
use roxy_vfs::{CreationMode, FilePermissions, OpenAccess, OpenOptions, VfsError};

use crate::{
    SyscallResult,
    args::{CString, SyscallArg, user_memory},
    errno::Errno,
    numbers::SyscallNumber,
    syscall,
};

syscall!(SyscallNumber::Open, handle(path_address: UserAddress => Fault, request: OpenRequest => Fault));

const OPEN_ACCESS_READ_ONLY: u32 = 0;
const OPEN_ACCESS_WRITE_ONLY: u32 = OPEN_ACCESS_READ_ONLY + 1;
const OPEN_ACCESS_READ_WRITE: u32 = OPEN_ACCESS_READ_ONLY + 2;

const OPEN_FLAGS_BASE: u64 = 1;

bitflags! {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct OpenFlags: u64 {
        const CREATE = OPEN_FLAGS_BASE;
        const EXCLUSIVE = OPEN_FLAGS_BASE << 1;
        const TRUNCATE = OPEN_FLAGS_BASE << 2;
        const APPEND = OPEN_FLAGS_BASE << 3;
        const NONBLOCK = OPEN_FLAGS_BASE << 4;
        const NOFOLLOW = OPEN_FLAGS_BASE << 5;
        const LARGE_FILE = OPEN_FLAGS_BASE << 6;
        const CLOEXEC = OPEN_FLAGS_BASE << 7;
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
struct OpenRequestAbi {
    access: u32,
    padding: u32,
    flags: u64,
    mode: u64,
}

const _: () = assert!(size_of::<OpenRequestAbi>() == 24);
const _: () = assert!(align_of::<OpenRequestAbi>() == 8);
const _: () = assert!(offset_of!(OpenRequestAbi, access) == 0);
const _: () = assert!(offset_of!(OpenRequestAbi, padding) == 4);
const _: () = assert!(offset_of!(OpenRequestAbi, flags) == 8);
const _: () = assert!(offset_of!(OpenRequestAbi, mode) == 16);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OpenRequest {
    access: OpenAccess,
    flags: OpenFlags,
    mode: u64,
}

impl SyscallArg for OpenRequest {
    fn parse(raw: u64, error: Errno) -> Result<Self, Errno> {
        let address = UserAddress::parse(raw, error)?;
        let mut abi = OpenRequestAbi {
            access: 0,
            padding: 0,
            flags: 0,
            mode: 0,
        };

        // SAFETY: OpenRequestAbi has a checked C layout, contains only integers, and accepts every
        // bit pattern copied from userspace.
        unsafe { user_memory::read(address, &mut abi) }?;

        let access = match abi.access {
            OPEN_ACCESS_READ_ONLY => OpenAccess::ReadOnly,
            OPEN_ACCESS_WRITE_ONLY => OpenAccess::WriteOnly,
            OPEN_ACCESS_READ_WRITE => OpenAccess::ReadWrite,
            value => return Err(unsupported("open.access", u64::from(value))),
        };

        let flags = OpenFlags::from_bits(abi.flags)
            .ok_or_else(|| unsupported("open.flags", abi.flags & !OpenFlags::all().bits()))?;

        Ok(Self {
            access,
            flags,
            mode: abi.mode,
        })
    }
}

impl OpenRequest {
    fn options(self) -> Result<OpenOptions, Errno> {
        let options = OpenOptions {
            access: self.access,
            creation: self.creation(),
            permissions: self.permissions()?,
            append: self.flags.contains(OpenFlags::APPEND),
            truncate: self.flags.contains(OpenFlags::TRUNCATE),
            no_follow: self.flags.contains(OpenFlags::NOFOLLOW),
        };

        options.validate().map_err(map_vfs_error)?;

        Ok(options)
    }

    fn creation(self) -> CreationMode {
        if self
            .flags
            .contains(OpenFlags::CREATE | OpenFlags::EXCLUSIVE)
        {
            CreationMode::CreateNew
        } else if self.flags.contains(OpenFlags::CREATE) {
            CreationMode::Create
        } else {
            CreationMode::OpenExisting
        }
    }

    fn permissions(self) -> Result<FilePermissions, Errno> {
        if !self.flags.contains(OpenFlags::CREATE) {
            return Ok(FilePermissions::DEFAULT_FILE);
        }

        let bits = u16::try_from(self.mode).map_err(|_| unsupported("open.mode", self.mode))?;

        FilePermissions::new(bits).ok_or_else(|| unsupported("open.mode", self.mode))
    }

    /// Returns the file status flags this open request implies, for `fcntl(F_GETFL)`.
    fn status_flags(self) -> StatusFlags {
        let access = match self.access {
            OpenAccess::ReadOnly => StatusFlags::empty().bits(),
            OpenAccess::WriteOnly => StatusFlags::WRITE_ONLY.bits(),
            OpenAccess::ReadWrite => StatusFlags::READ_WRITE.bits(),
        };
        let extra = self.flags.bits()
            & (StatusFlags::APPEND.bits()
                | StatusFlags::LARGE_FILE.bits()
                | StatusFlags::NONBLOCK.bits());

        StatusFlags::from_bits_retain(access | extra)
    }
}

fn handle(path_address: UserAddress, request: OpenRequest) -> SyscallResult {
    let path = CString::from_address(path_address)?;

    if path.is_empty() {
        return Err(Errno::NotFound);
    }

    let options = request.options()?;

    let file = roxy_vfs::open(path.into_inner(), options).map_err(map_vfs_error)?;
    let file = OpenFile::new(Box::new(file));
    file.set_status_flags(request.status_flags());
    let fd = roxy_process::insert_open_file(file, request.flags.contains(OpenFlags::CLOEXEC));

    Ok(u64::from(fd.as_u32()))
}

fn map_vfs_error(error: VfsError) -> Errno {
    match error {
        VfsError::NotInitialized | VfsError::Io | VfsError::Corrupt => Errno::Io,
        VfsError::InvalidPath | VfsError::InvalidInput | VfsError::DirectoryNotEmpty => {
            Errno::Invalid
        }
        VfsError::NotFound => Errno::NotFound,
        VfsError::AlreadyExists => Errno::AlreadyExists,
        VfsError::NotDirectory => Errno::NotDirectory,
        VfsError::IsDirectory => Errno::IsDirectory,
        VfsError::ReadOnly => Errno::ReadOnly,
        VfsError::PermissionDenied => Errno::Access,
        VfsError::NoSpace => Errno::NoSpace,
        VfsError::Busy => Errno::Busy,
        VfsError::CrossDevice => Errno::CrossDevice,
        VfsError::Loop => Errno::Loop,
        VfsError::Unsupported => unsupported("open.filesystem", 0),
        VfsError::WouldBlock => Errno::Again,
    }
}

fn unsupported(operation: &str, argument: u64) -> Errno {
    crate::unsupported::unsupported_argument(operation, argument, Errno::NotSupported)
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_test::kernel_test;
    use roxy_vfs::{CreationMode, FilePermissions, OpenAccess};

    use super::{OpenFlags, OpenRequest};

    use crate::errno::Errno;

    kernel_test!(
        "roxy-syscall::open-options",
        converts_supported_open_flags,
        {
            let flags = OpenFlags::CREATE
                | OpenFlags::EXCLUSIVE
                | OpenFlags::TRUNCATE
                | OpenFlags::LARGE_FILE;
            let request = OpenRequest {
                access: OpenAccess::ReadWrite,
                flags,
                mode: 0o640,
            };
            let options = request.options().unwrap();

            assert_eq!(options.access, OpenAccess::ReadWrite);
            assert_eq!(options.creation, CreationMode::CreateNew);
            assert_eq!(options.permissions, FilePermissions::new(0o640).unwrap());
            assert!(options.truncate);
            assert!(!options.append);
        }
    );

    kernel_test!(
        "roxy-syscall::invalid-open-options",
        rejects_invalid_open_flags,
        {
            let read_only = OpenRequest {
                access: OpenAccess::ReadOnly,
                flags: OpenFlags::APPEND,
                mode: 0,
            };
            assert_eq!(read_only.options(), Err(Errno::Invalid));
        }
    );

    kernel_test!("roxy-syscall::open-status-flags", reports_open_mode, {
        use roxy_fd::StatusFlags;

        let read_only = OpenRequest {
            access: OpenAccess::ReadOnly,
            flags: OpenFlags::empty(),
            mode: 0,
        };
        let write_only = OpenRequest {
            access: OpenAccess::WriteOnly,
            flags: OpenFlags::empty(),
            mode: 0,
        };
        let read_write = OpenRequest {
            access: OpenAccess::ReadWrite,
            flags: OpenFlags::APPEND | OpenFlags::LARGE_FILE,
            mode: 0,
        };
        let read_write_nonblocking = OpenRequest {
            access: OpenAccess::ReadWrite,
            flags: OpenFlags::NONBLOCK,
            mode: 0,
        };

        assert_eq!(read_only.status_flags(), StatusFlags::empty());
        assert_eq!(write_only.status_flags(), StatusFlags::WRITE_ONLY);
        assert_eq!(
            read_write.status_flags(),
            StatusFlags::READ_WRITE | StatusFlags::APPEND | StatusFlags::LARGE_FILE
        );
        assert_eq!(
            read_write_nonblocking.status_flags(),
            StatusFlags::READ_WRITE | StatusFlags::NONBLOCK
        );
    });
}
