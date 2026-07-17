use heapless::{String, Vec};
use limine::{
    BaseRevision, RequestsEndMarker, RequestsStartMarker, firmware, memmap,
    request::{
        BootloaderInfoRequest, ExecutableAddressRequest, ExecutableCmdlineRequest,
        FirmwareTypeRequest, FramebufferRequest, HhdmRequest, MemmapRequest, RsdpRequest,
        StackSizeRequest,
    },
};
use roxy_arch::CpuId;

use super::{Bootloader, sealed};
use crate::{
    BootInfo, FramebufferInfo, KernelAddressInfo, MAX_FRAMEBUFFERS, MAX_MEMORY_REGIONS,
    MemoryRegion, MemoryRegionKind,
};

#[used]
#[unsafe(link_section = ".limine_requests_start")]
static REQUESTS_START: RequestsStartMarker = RequestsStartMarker::new();
#[used]
#[unsafe(link_section = ".limine_requests")]
static BASE_REVISION: BaseRevision = BaseRevision::new();
#[used]
#[unsafe(link_section = ".limine_requests")]
static STACK_SIZE: StackSizeRequest = StackSizeRequest::new(64 * 1024);
#[used]
#[unsafe(link_section = ".limine_requests")]
static FIRMWARE: FirmwareTypeRequest = FirmwareTypeRequest::new();
#[used]
#[unsafe(link_section = ".limine_requests")]
static BOOTLOADER: BootloaderInfoRequest = BootloaderInfoRequest::new();
#[used]
#[unsafe(link_section = ".limine_requests")]
static CMDLINE: ExecutableCmdlineRequest = ExecutableCmdlineRequest::new();
#[used]
#[unsafe(link_section = ".limine_requests")]
static MEMMAP: MemmapRequest = MemmapRequest::new();
#[used]
#[unsafe(link_section = ".limine_requests")]
static HHDM: HhdmRequest = HhdmRequest::new();
#[used]
#[unsafe(link_section = ".limine_requests")]
static EXECUTABLE_ADDRESS: ExecutableAddressRequest = ExecutableAddressRequest::new();
#[used]
#[unsafe(link_section = ".limine_requests")]
static FRAMEBUFFER: FramebufferRequest = FramebufferRequest::new();
#[used]
#[unsafe(link_section = ".limine_requests")]
static RSDP: RsdpRequest = RsdpRequest::new();
#[used]
#[unsafe(link_section = ".limine_requests_end")]
static REQUESTS_END: RequestsEndMarker = RequestsEndMarker::new();

pub struct Limine;

impl sealed::Sealed for Limine {}

impl Bootloader for Limine {
    fn parse() -> BootInfo {
        validate_environment();
        load_boot_info()
    }
}

fn validate_environment() {
    assert!(BASE_REVISION.is_supported());

    let firmware = FIRMWARE.response().unwrap();
    assert_eq!(firmware.firmware_type, firmware::FIRMWARE_TYPE_EFI64);

    STACK_SIZE.response().unwrap();
}

fn load_boot_info() -> BootInfo {
    let loader = BOOTLOADER.response().unwrap();
    let address = EXECUTABLE_ADDRESS.response().unwrap();

    BootInfo {
        memory_regions: memory_regions(),
        framebuffers: framebuffers(),
        hhdm_offset: HHDM.response().unwrap().offset,
        kernel_address: KernelAddressInfo {
            physical_base: address.physical_base,
            virtual_base: address.virtual_base,
        },
        rsdp_address: RSDP.response().unwrap().address as u64,
        command_line: copy_string(CMDLINE.response().unwrap().cmdline()),
        bootloader_name: copy_string(loader.name()),
        bootloader_version: copy_string(loader.version()),
        bsp: CpuId::BSP,
    }
}

fn memory_regions() -> Vec<MemoryRegion, MAX_MEMORY_REGIONS> {
    MEMMAP
        .response()
        .unwrap()
        .entries()
        .iter()
        .map(|entry| MemoryRegion {
            base: entry.base,
            length: entry.length,
            kind: map_memory_kind(entry.type_),
        })
        .collect()
}

fn framebuffers() -> Vec<FramebufferInfo, MAX_FRAMEBUFFERS> {
    let framebuffers: Vec<FramebufferInfo, MAX_FRAMEBUFFERS> = FRAMEBUFFER
        .response()
        .unwrap()
        .framebuffers()
        .iter()
        .map(|framebuffer| FramebufferInfo {
            address: framebuffer.address() as u64,
            width: framebuffer.width,
            height: framebuffer.height,
            pitch: framebuffer.pitch,
            bits_per_pixel: framebuffer.bpp,
        })
        .collect();

    (!framebuffers.is_empty()).then_some(framebuffers).unwrap()
}

fn copy_string<const SIZE: usize>(value: &str) -> String<SIZE> {
    let mut output = String::new();
    output.push_str(value).unwrap();
    output
}

fn map_memory_kind(value: u64) -> MemoryRegionKind {
    match value {
        memmap::MEMMAP_USABLE => MemoryRegionKind::Usable,
        memmap::MEMMAP_RESERVED => MemoryRegionKind::Reserved,
        memmap::MEMMAP_ACPI_RECLAIMABLE => MemoryRegionKind::AcpiReclaimable,
        memmap::MEMMAP_ACPI_NVS => MemoryRegionKind::AcpiNvs,
        memmap::MEMMAP_BAD_MEMORY => MemoryRegionKind::BadMemory,
        memmap::MEMMAP_BOOTLOADER_RECLAIMABLE => MemoryRegionKind::BootloaderReclaimable,
        memmap::MEMMAP_EXECUTABLE_AND_MODULES => MemoryRegionKind::ExecutableAndModules,
        memmap::MEMMAP_FRAMEBUFFER => MemoryRegionKind::Framebuffer,
        memmap::MEMMAP_MAPPED_RESERVED => MemoryRegionKind::MappedReserved,
        other => MemoryRegionKind::Unknown(other),
    }
}
