#![no_std]
#![allow(dead_code)]

mod loader;

use core::fmt;

use heapless::{String, Vec};
use roxy_arch::CpuId;

pub use loader::{Bootloader, CurrentLoader, Limine};

pub const MAX_FRAMEBUFFERS: usize = 8;
pub const MAX_MEMORY_REGIONS: usize = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryRegionKind {
    Usable,
    Reserved,
    AcpiReclaimable,
    AcpiNvs,
    BadMemory,
    BootloaderReclaimable,
    ExecutableAndModules,
    Framebuffer,
    MappedReserved,
    Unknown(u64),
}

impl fmt::Display for MemoryRegionKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usable => formatter.write_str("usable"),
            Self::Reserved => formatter.write_str("reserved"),
            Self::AcpiReclaimable => formatter.write_str("acpi-reclaimable"),
            Self::AcpiNvs => formatter.write_str("acpi-nvs"),
            Self::BadMemory => formatter.write_str("bad-memory"),
            Self::BootloaderReclaimable => formatter.write_str("bootloader-reclaimable"),
            Self::ExecutableAndModules => formatter.write_str("executable-and-modules"),
            Self::Framebuffer => formatter.write_str("framebuffer"),
            Self::MappedReserved => formatter.write_str("mapped-reserved"),
            Self::Unknown(value) => write!(formatter, "unknown({value:#x})"),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MemoryRegion {
    pub base: u64,
    pub length: u64,
    pub kind: MemoryRegionKind,
}

#[derive(Clone, Copy, Debug)]
pub struct KernelAddressInfo {
    pub physical_base: u64,
    pub virtual_base: u64,
}

#[derive(Clone, Copy, Debug)]
pub struct FramebufferInfo {
    pub address: u64,
    pub width: u64,
    pub height: u64,
    pub pitch: u64,
    pub bits_per_pixel: u16,
}

pub struct BootInfo {
    pub memory_regions: Vec<MemoryRegion, MAX_MEMORY_REGIONS>,
    pub framebuffers: Vec<FramebufferInfo, MAX_FRAMEBUFFERS>,
    pub hhdm_offset: u64,
    pub kernel_address: KernelAddressInfo,
    pub rsdp_address: u64,
    pub command_line: String<256>,
    pub bootloader_name: String<64>,
    pub bootloader_version: String<64>,
    pub bsp: CpuId,
    pub unix_seconds_at_boot: u64,
}

impl BootInfo {
    #[must_use]
    pub fn parse() -> Self {
        CurrentLoader::parse()
    }
}
