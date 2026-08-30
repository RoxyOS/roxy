use alloc::boxed::Box;

use bitflags::bitflags;
use roxy_fd::{OpenFile, StatusFlags};
use roxy_memory::UserAddress;
use roxy_vfs::{CreationMode, FilePermissions, OpenAccess, OpenOptions, VfsError};

use crate::{
    SyscallResult,
    args::{CString, SyscallArg},
    errno::Errno,
    numbers::SyscallNumber,
    syscall,
};

syscall!(SyscallNumber::Open, handle(path_address: UserAddress => Fault, flags: OpenFlags => Invalid, mode: u64));

const ACCESS_MASK: u64 = 0o3;

bitflags! {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct OpenFlags: u64 {
        const WRITE_ONLY = 0o1;
        const READ_WRITE = 0o2;
        const CREATE = 0o100;
        const EXCLUSIVE = 0o200;
        const TRUNCATE = 0o1000;
        const APPEND = 0o2000;
        const LARGE_FILE = 0o100_000;
        const CLOEXEC = 0o2_000_000;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OpenRequest {
    flags: OpenFlags,
    mode: u64,
}

impl OpenRequest {
    const fn new(flags: OpenFlags, mode: u64) -> Self {
        Self { flags, mode }
    }

    fn options(self) -> Result<OpenOptions, Errno> {
        let options = OpenOptions {
            access: self.access()?,
            creation: self.creation(),
            permissions: self.permissions()?,
            append: self.flags.contains(OpenFlags::APPEND),
            truncate: self.flags.contains(OpenFlags::TRUNCATE),
        };

        options.validate().map_err(map_vfs_error)?;

        Ok(options)
    }

    fn access(self) -> Result<OpenAccess, Errno> {
        let access = match self.flags.bits() & ACCESS_MASK {
            0 => OpenAccess::ReadOnly,
            1 => OpenAccess::WriteOnly,
            2 => OpenAccess::ReadWrite,
            _ => return Err(Errno::Invalid),
        };

        Ok(access)
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
        // `OpenRequest::options` already rejects an invalid access mode (3), so only 1 and 2
        // can reach here; anything else stays read-only.
        let access = match self.flags.bits() & ACCESS_MASK {
            1 => StatusFlags::WRITE_ONLY.bits(),
            2 => StatusFlags::READ_WRITE.bits(),
            _ => 0,
        };
        let extra =
            self.flags.bits() & (StatusFlags::APPEND.bits() | StatusFlags::LARGE_FILE.bits());

        StatusFlags::from_bits_retain(access | extra)
    }
}

impl SyscallArg for OpenFlags {
    fn parse(raw: u64, _error: Errno) -> Result<Self, Errno> {
        let unknown = raw & !Self::all().bits();

        if unknown != 0 {
            return Err(unsupported("open.flags", unknown));
        }

        Ok(Self::from_bits_retain(raw))
    }
}

fn handle(path_address: UserAddress, flags: OpenFlags, mode: u64) -> SyscallResult {
    let request = OpenRequest::new(flags, mode);

    let path = CString::from_address(path_address)?;

    if path.is_empty() {
        return Err(Errno::NotFound);
    }

    let options = request.options()?;

    let file = roxy_vfs::open(path.into_inner(), options).map_err(map_vfs_error)?;
    let file = OpenFile::new(Box::new(file));
    file.set_status_flags(request.status_flags());
    let fd = roxy_process::insert_open_file(file, flags.contains(OpenFlags::CLOEXEC));

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
        VfsError::Unsupported => unsupported("open.filesystem", 0),
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
            let flags = OpenFlags::READ_WRITE
                | OpenFlags::CREATE
                | OpenFlags::EXCLUSIVE
                | OpenFlags::TRUNCATE
                | OpenFlags::LARGE_FILE;
            let options = OpenRequest::new(flags, 0o640).options().unwrap();

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
            let invalid_access = OpenRequest::new(OpenFlags::from_bits_retain(0o3), 0);
            let read_only_append = OpenRequest::new(OpenFlags::APPEND, 0);

            assert_eq!(invalid_access.options(), Err(Errno::Invalid));
            assert_eq!(read_only_append.options(), Err(Errno::Invalid));
        }
    );

    kernel_test!("roxy-syscall::open-status-flags", reports_open_mode, {
        use roxy_fd::StatusFlags;

        let read_only = OpenRequest::new(OpenFlags::empty(), 0);
        let write_only = OpenRequest::new(OpenFlags::WRITE_ONLY, 0);
        let read_write = OpenRequest::new(
            OpenFlags::READ_WRITE | OpenFlags::APPEND | OpenFlags::LARGE_FILE,
            0,
        );

        assert_eq!(read_only.status_flags(), StatusFlags::empty());
        assert_eq!(write_only.status_flags(), StatusFlags::WRITE_ONLY);
        assert_eq!(
            read_write.status_flags(),
            StatusFlags::READ_WRITE | StatusFlags::APPEND | StatusFlags::LARGE_FILE
        );
    });
}
