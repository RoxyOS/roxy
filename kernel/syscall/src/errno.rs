#[repr(u64)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Errno {
    NoSys = 38,
}

impl Errno {
    #[must_use]
    pub(crate) const fn encode(self) -> u64 {
        (-(self as i64)).cast_unsigned()
    }
}

#[cfg(test)]
mod tests {
    use super::Errno;

    #[test]
    fn errno_encodes_as_negative_return() {
        assert_eq!(Errno::NoSys.encode().cast_signed(), -38);
    }
}
