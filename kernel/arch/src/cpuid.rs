use core::fmt;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CpuId(u32);

impl CpuId {
    pub const BSP: Self = Self(0);

    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

impl From<u32> for CpuId {
    fn from(value: u32) -> Self {
        Self::new(value)
    }
}

impl From<CpuId> for u32 {
    fn from(cpu_id: CpuId) -> Self {
        cpu_id.get()
    }
}

impl fmt::Display for CpuId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}
