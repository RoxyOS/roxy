use alloc::vec::Vec;
use core::fmt;
use spin::Once;

use crate::VfsError;

/// A normalized absolute path ready for mount routing and filesystem operations.
#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct ResolvedPath(Vec<u8>);

/// Supplies the normalized absolute working directory of the current process.
pub type WorkingDirectoryProvider = fn() -> ResolvedPath;

static WORKING_DIRECTORY_PROVIDER: Once<WorkingDirectoryProvider> = Once::new();

pub fn register_working_directory_provider(
    provider: WorkingDirectoryProvider,
) -> Result<(), VfsError> {
    if WORKING_DIRECTORY_PROVIDER.get().is_some() {
        return Err(VfsError::Busy);
    }

    WORKING_DIRECTORY_PROVIDER.call_once(|| provider);

    Ok(())
}

impl ResolvedPath {
    pub const MAX_LEN: usize = 4096;
    pub const MAX_COMPONENT_LEN: usize = 255;

    /// Resolves raw absolute or process-relative path bytes into a normalized absolute path.
    pub fn resolve(path: impl AsRef<[u8]>) -> Result<Self, VfsError> {
        let path = path.as_ref();
        if path.is_empty() || path.contains(&0) {
            return Err(VfsError::InvalidPath);
        }

        if path.first() == Some(&b'/') {
            return Self::normalize(path, false);
        }

        let provider = WORKING_DIRECTORY_PROVIDER
            .get()
            .ok_or(VfsError::NotInitialized)?;
        let working_directory = provider();

        Self::with_base(path, &working_directory)
    }

    /// Applies an absolute base directory to raw caller path bytes and normalizes the result.
    ///
    /// Absolute paths ignore `base`; relative paths are appended to `base`. The returned
    /// `ResolvedPath` is always absolute.
    pub(crate) fn with_base(path: impl AsRef<[u8]>, base: &Self) -> Result<Self, VfsError> {
        let path = path.as_ref();
        if path.first() == Some(&b'/') {
            return Self::normalize(path, false);
        }
        if path.contains(&0) {
            return Err(VfsError::InvalidPath);
        }

        let mut absolute = base.0.clone();
        absolute.push(b'/');
        absolute.extend_from_slice(path);

        Self::normalize(&absolute, true)
    }

    fn normalize(path: &[u8], stay_at_root: bool) -> Result<Self, VfsError> {
        let mut components: Vec<&[u8]> = Vec::new();
        for component in path.split(|byte| *byte == b'/') {
            match component {
                b"" | b"." => {}
                b".." => {
                    if components.pop().is_none() && !stay_at_root {
                        return Err(VfsError::InvalidPath);
                    }
                }
                value if value.len() > Self::MAX_COMPONENT_LEN => {
                    return Err(VfsError::InvalidPath);
                }
                value => components.push(value),
            }
        }

        let length = 1
            + components.iter().map(|value| value.len()).sum::<usize>()
            + components.len().saturating_sub(1);
        if length > Self::MAX_LEN {
            return Err(VfsError::InvalidPath);
        }

        let mut normalized = Vec::with_capacity(length);
        normalized.push(b'/');
        for (index, component) in components.into_iter().enumerate() {
            if index != 0 {
                normalized.push(b'/');
            }
            normalized.extend_from_slice(component);
        }

        Ok(Self(normalized))
    }

    #[must_use]
    pub fn root() -> Self {
        Self(alloc::vec![b'/'])
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    #[must_use]
    pub fn is_root(&self) -> bool {
        self.0 == b"/"
    }

    #[must_use]
    pub fn contains(&self, path: &Self) -> bool {
        self.is_root()
            || path == self
            || (path.0.starts_with(&self.0) && path.0.get(self.0.len()) == Some(&b'/'))
    }

    #[must_use]
    pub fn relative_to(&self, mount: &Self) -> Self {
        if mount.is_root() {
            return self.clone();
        }
        if self == mount {
            return Self::root();
        }

        Self(self.0[mount.0.len()..].to_vec())
    }
}

impl fmt::Debug for ResolvedPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", StringBytes(&self.0))
    }
}

struct StringBytes<'a>(&'a [u8]);

impl fmt::Display for StringBytes<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            for escaped in core::ascii::escape_default(*byte) {
                formatter.write_str(core::str::from_utf8(&[escaped]).unwrap_or("?"))?;
            }
        }

        Ok(())
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use super::ResolvedPath;
    use crate::VfsError;

    roxy_test::kernel_test!(
        "roxy-vfs::normalizes-absolute-byte-paths",
        normalizes_absolute_byte_paths,
        {
            assert_eq!(
                ResolvedPath::resolve(b"//usr/./lib/../bin/")
                    .unwrap()
                    .as_bytes(),
                b"/usr/bin"
            );
            assert_eq!(
                ResolvedPath::resolve(b"/../../etc"),
                Err(VfsError::InvalidPath)
            );
            assert_eq!(ResolvedPath::resolve(b""), Err(VfsError::InvalidPath));
        }
    );

    roxy_test::kernel_test!(
        "roxy-vfs::resolves-relative-byte-paths",
        resolves_relative_byte_paths,
        {
            let root = ResolvedPath::root();
            let working_directory = ResolvedPath::resolve(b"/usr/lib").unwrap();

            assert_eq!(
                ResolvedPath::with_base(b".", &root).unwrap().as_bytes(),
                b"/"
            );
            assert_eq!(
                ResolvedPath::with_base(b"..", &root).unwrap().as_bytes(),
                b"/"
            );
            assert_eq!(
                ResolvedPath::with_base(b"foo", &root).unwrap().as_bytes(),
                b"/foo"
            );
            assert_eq!(
                ResolvedPath::with_base(b"./child", &working_directory)
                    .unwrap()
                    .as_bytes(),
                b"/usr/lib/child"
            );
            assert_eq!(
                ResolvedPath::with_base(b"../bin", &working_directory)
                    .unwrap()
                    .as_bytes(),
                b"/usr/bin"
            );
            assert_eq!(
                ResolvedPath::with_base(b"/etc", &working_directory)
                    .unwrap()
                    .as_bytes(),
                b"/etc"
            );
        }
    );

    roxy_test::kernel_test!(
        "roxy-vfs::rejects-invalid-relative-byte-paths",
        rejects_invalid_relative_byte_paths,
        {
            let working_directory = ResolvedPath::resolve(b"/usr").unwrap();
            let long_component = [b'a'; ResolvedPath::MAX_COMPONENT_LEN + 1];
            let long_path = alloc::vec![b'a'; ResolvedPath::MAX_LEN];

            assert_eq!(
                ResolvedPath::with_base(b"child\0name", &working_directory),
                Err(VfsError::InvalidPath)
            );
            assert_eq!(
                ResolvedPath::with_base(long_component, &working_directory),
                Err(VfsError::InvalidPath)
            );
            assert_eq!(
                ResolvedPath::with_base(long_path, &working_directory),
                Err(VfsError::InvalidPath)
            );
            assert_eq!(
                ResolvedPath::with_base(b"/../../etc", &working_directory),
                Err(VfsError::InvalidPath)
            );
        }
    );

    roxy_test::kernel_test!(
        "roxy-vfs::matches-component-boundaries",
        matches_component_boundaries,
        {
            let mount = ResolvedPath::resolve(b"/mnt").unwrap();

            assert!(mount.contains(&ResolvedPath::resolve(b"/mnt/a").unwrap()));
            assert!(!mount.contains(&ResolvedPath::resolve(b"/mnt2").unwrap()));
        }
    );
}
