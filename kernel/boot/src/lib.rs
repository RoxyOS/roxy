#![no_std]
#![allow(dead_code)]

mod loader;

use core::fmt;

use heapless::{String, Vec};
use roxy_arch::CpuId;

pub use loader::{Bootloader, CurrentLoader, Limine};

const MAX_FRAMEBUFFERS: usize = 8;
const MAX_MEMORY_REGIONS: usize = 256;

#[derive(Clone, Copy, Debug)]
enum MemoryRegionKind {
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
struct MemoryRegion {
    base: u64,
    length: u64,
    kind: MemoryRegionKind,
}

#[derive(Clone, Copy, Debug)]
struct KernelAddressInfo {
    physical_base: u64,
    virtual_base: u64,
}

#[derive(Clone, Copy, Debug)]
struct FramebufferInfo {
    address: u64,
    width: u64,
    height: u64,
    pitch: u64,
    bits_per_pixel: u16,
}

pub struct BootInfo {
    memory_regions: Vec<MemoryRegion, MAX_MEMORY_REGIONS>,
    framebuffers: Vec<FramebufferInfo, MAX_FRAMEBUFFERS>,
    hhdm_offset: u64,
    kernel_address: KernelAddressInfo,
    rsdp_address: u64,
    command_line: String<256>,
    bootloader_name: String<64>,
    bootloader_version: String<64>,
    bsp: CpuId,
}

impl BootInfo {
    #[must_use]
    pub fn parse() -> Self {
        CurrentLoader::parse()
    }
}
