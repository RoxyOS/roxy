use alloc::vec::Vec;
use core::fmt;

use crate::VfsError;

#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct VfsPath(Vec<u8>);

impl VfsPath {
    pub const MAX_LEN: usize = 4096;
    pub const MAX_COMPONENT_LEN: usize = 255;

    pub fn new(path: impl AsRef<[u8]>) -> Result<Self, VfsError> {
        let path = path.as_ref();
        if path.first() != Some(&b'/') || path.contains(&0) {
            return Err(VfsError::InvalidPath);
        }

        let mut components: Vec<&[u8]> = Vec::new();
        for component in path.split(|byte| *byte == b'/') {
            match component {
                b"" | b"." => {}
                b".." => {
                    components.pop().ok_or(VfsError::InvalidPath)?;
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

impl fmt::Debug for VfsPath {
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
    use super::VfsPath;
    use crate::VfsError;

    roxy_test::kernel_test!(
        "roxy-vfs::normalizes-absolute-byte-paths",
        normalizes_absolute_byte_paths,
        {
            assert_eq!(
                VfsPath::new(b"//usr/./lib/../bin/").unwrap().as_bytes(),
                b"/usr/bin"
            );
            assert_eq!(VfsPath::new(b"/../../etc"), Err(VfsError::InvalidPath));
            assert_eq!(VfsPath::new(b"relative"), Err(VfsError::InvalidPath));
        }
    );

    roxy_test::kernel_test!(
        "roxy-vfs::matches-component-boundaries",
        matches_component_boundaries,
        {
            let mount = VfsPath::new(b"/mnt").unwrap();

            assert!(mount.contains(&VfsPath::new(b"/mnt/a").unwrap()));
            assert!(!mount.contains(&VfsPath::new(b"/mnt2").unwrap()));
        }
    );
}
