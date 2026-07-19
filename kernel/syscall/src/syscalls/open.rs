use bitflags::bitflags;
use roxy_memory::UserAddress;
use roxy_vfs::{CreationMode, FilePermissions, OpenAccess, OpenOptions, VfsError, VfsPath};

use crate::{Syscall, SyscallResult, errno::Errno, numbers::SyscallNumber};

pub(super) const SYSCALL: Syscall = Syscall::new(SyscallNumber::Open, handle);

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
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OpenRequest {
    flags: OpenFlags,
    mode: u64,
}

impl OpenRequest {
    fn parse(flags: u64, mode: u64) -> Result<Self, Errno> {
        let unknown = flags & !OpenFlags::all().bits();

        if unknown != 0 {
            return Err(unsupported("open.flags", unknown));
        }

        Ok(Self {
            flags: OpenFlags::from_bits_retain(flags),
            mode,
        })
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
}

fn handle(arguments: [u64; 6]) -> SyscallResult {
    let path_address = UserAddress::new(arguments[0]).ok_or(Errno::Fault)?;
    let request = OpenRequest::parse(arguments[1], arguments[2])?;

    let addrspace = roxy_process::current_addrspace().map_err(|_| Errno::Fault)?;
    let path = crate::user::read_c_string(&addrspace, path_address, VfsPath::MAX_LEN)?;

    if path.is_empty() {
        return Err(Errno::NotFound);
    }

    if path.first() != Some(&b'/') {
        return Err(unsupported("open.relative_path", 0));
    }

    let options = request.options()?;

    let file = roxy_vfs::open(path, options).map_err(map_vfs_error)?;
    let fd = roxy_process::insert_open_file(roxy_fd::OpenFile::from_vfs(file));

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
        VfsError::Unsupported => Errno::NotSupported,
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
            let options = OpenRequest::parse(flags.bits(), 0o640)
                .unwrap()
                .options()
                .unwrap();

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
            let invalid_access = OpenRequest::parse(0o3, 0).unwrap();
            let read_only_append = OpenRequest::parse(OpenFlags::APPEND.bits(), 0).unwrap();

            assert_eq!(invalid_access.options(), Err(Errno::Invalid));
            assert_eq!(read_only_append.options(), Err(Errno::Invalid));
        }
    );
}
