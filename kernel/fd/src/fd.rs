#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Fd(u32);

impl Fd {
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use super::Fd;
    use roxy_test::kernel_test;

    kernel_test!("roxy-fd::descriptor-value", preserves_descriptor_number, {
        assert_eq!(Fd::new(7), Fd::new(7));
        assert_ne!(Fd::new(7), Fd::new(8));
    });
}
