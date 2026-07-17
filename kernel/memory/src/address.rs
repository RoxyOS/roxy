use core::fmt;

pub const PAGE_SIZE: u64 = 4096;
pub const USER_ADDRESS_MIN: u64 = 0x0000_0000_0001_0000;
pub const USER_ADDRESS_MAX: u64 = 0x0000_7fff_ffff_f000;

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

pub(crate) const fn align_down(value: u64) -> u64 {
    value & !(PAGE_SIZE - 1)
}

pub(crate) const fn align_up(value: u64) -> Option<u64> {
    match value.checked_add(PAGE_SIZE - 1) {
        Some(value) => Some(align_down(value)),
        None => None,
    }
}
