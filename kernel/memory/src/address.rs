use core::fmt;

pub const PAGE_SIZE: u64 = 4096;
pub const USER_ADDRESS_MIN: u64 = 0x0000_0000_0001_0000;
pub const USER_ADDRESS_MAX: u64 = 0x0000_7fff_ffff_ffff;

macro_rules! address_type {
    ($name:ident) => {
        #[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
        pub struct $name(u64);

        impl $name {
            #[must_use]
            pub const fn as_u64(self) -> u64 {
                self.0
            }

            #[must_use]
            pub const fn checked_add(self, offset: u64) -> Option<Self> {
                match self.0.checked_add(offset) {
                    Some(value) => Self::new(value),
                    None => None,
                }
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(formatter, "{}({:#x})", stringify!($name), self.0)
            }
        }
    };
}

address_type!(PhysicalAddress);
address_type!(VirtualAddress);
address_type!(UserAddress);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct UserPage(UserAddress);

impl PhysicalAddress {
    #[must_use]
    pub const fn new(value: u64) -> Option<Self> {
        if value < (1 << 52) {
            Some(Self(value))
        } else {
            None
        }
    }
}

impl VirtualAddress {
    #[must_use]
    pub const fn new(value: u64) -> Option<Self> {
        let sign = (value >> 47) & 1;
        let upper = value >> 48;
        if (sign == 0 && upper == 0) || (sign == 1 && upper == 0xffff) {
            Some(Self(value))
        } else {
            None
        }
    }
}

impl UserAddress {
    #[must_use]
    pub const fn new(value: u64) -> Option<Self> {
        if value >= USER_ADDRESS_MIN && value <= USER_ADDRESS_MAX {
            Some(Self(value))
        } else {
            None
        }
    }
}

impl UserPage {
    #[must_use]
    pub const fn new(address: UserAddress) -> Option<Self> {
        if address.as_u64().is_multiple_of(PAGE_SIZE) {
            Some(Self(address))
        } else {
            None
        }
    }

    #[must_use]
    pub const fn containing(address: UserAddress) -> Self {
        Self(UserAddress(align_down(address.as_u64())))
    }

    #[must_use]
    pub const fn start_address(self) -> UserAddress {
        self.0
    }

    #[must_use]
    pub fn checked_add(self, page_count: usize) -> Option<Self> {
        let byte_count = u64::try_from(page_count).ok()?.checked_mul(PAGE_SIZE)?;
        Self::new(self.0.checked_add(byte_count)?)
    }
}

pub(crate) const fn align_down(value: u64) -> u64 {
    value & !(PAGE_SIZE - 1)
}

pub(crate) const fn align_up(value: u64) -> Option<u64> {
    match value.checked_add(PAGE_SIZE - 1) {
        Some(value) => Some(align_down(value)),
        None => None,
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use super::{
        PAGE_SIZE, PhysicalAddress, USER_ADDRESS_MAX, USER_ADDRESS_MIN, UserAddress,
        VirtualAddress, align_down, align_up,
    };

    roxy_test::kernel_test!(
        "roxy-memory::physical-address-bounds",
        physical_address_bounds,
        {
            let maximum = (1 << 52) - 1;

            assert_eq!(PhysicalAddress::new(maximum).unwrap().as_u64(), maximum);
            assert!(PhysicalAddress::new(1 << 52).is_none());
            assert!(
                PhysicalAddress::new(maximum)
                    .unwrap()
                    .checked_add(1)
                    .is_none()
            );
        }
    );

    roxy_test::kernel_test!(
        "roxy-memory::virtual-address-bounds",
        virtual_address_bounds,
        {
            assert!(VirtualAddress::new(0x0000_7fff_ffff_ffff).is_some());
            assert!(VirtualAddress::new(0x0000_8000_0000_0000).is_none());
            assert!(VirtualAddress::new(0xffff_8000_0000_0000).is_some());
            assert!(VirtualAddress::new(0xffff_7fff_ffff_ffff).is_none());
        }
    );

    roxy_test::kernel_test!("roxy-memory::user-address-bounds", user_address_bounds, {
        assert!(UserAddress::new(USER_ADDRESS_MIN).is_some());
        assert!(UserAddress::new(USER_ADDRESS_MAX).is_some());
        assert!(UserAddress::new(USER_ADDRESS_MIN - 1).is_none());
        assert!(UserAddress::new(USER_ADDRESS_MAX + 1).is_none());
    });

    roxy_test::kernel_test!("roxy-memory::page-alignment", page_alignment, {
        assert_eq!(align_down(PAGE_SIZE + 1), PAGE_SIZE);
        assert_eq!(align_up(PAGE_SIZE + 1), Some(PAGE_SIZE * 2));
        assert_eq!(align_up(PAGE_SIZE), Some(PAGE_SIZE));
        assert_eq!(align_up(u64::MAX), None);
    });
}
