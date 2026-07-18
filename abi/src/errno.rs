use core::mem::{align_of, size_of};

#[repr(u64)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Errno {
    NoSys = 38,
}

impl Errno {
    #[must_use]
    pub const fn encode(self) -> u64 {
        (-(self as i64)).cast_unsigned()
    }
}

const _: () = {
    assert!(size_of::<Errno>() == 8);
    assert!(align_of::<Errno>() == 8);
};

#[cfg(test)]
mod tests {
    use super::Errno;

    #[test]
    fn errno_encodes_as_negative_return() {
        assert_eq!(Errno::NoSys.encode().cast_signed(), -38);
    }
}
