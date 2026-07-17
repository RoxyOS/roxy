mod x86_64;

pub(super) use self::x86_64::X86_64Cpu;

pub(super) type CurrentCpuArchitecture = X86_64Cpu;

pub(super) trait CpuArchitecture: sealed::Sealed {
    fn initialize() -> u32;
}

mod sealed {
    pub trait Sealed {}
}
