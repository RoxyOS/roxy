use heapless::{String, Vec};
use limine::{
    BaseRevision, RequestsEndMarker, RequestsStartMarker, firmware, memmap,
    request::{
        BootloaderInfoRequest, DateAtBootRequest, ExecutableAddressRequest,
        ExecutableCmdlineRequest, FirmwareTypeRequest, FramebufferRequest, HhdmRequest,
        MemmapRequest, ModulesRequest, RsdpRequest, StackSizeRequest,
    },
};
use roxy_arch::CpuId;

use super::{Bootloader, sealed};
use crate::{
    BootInfo, FramebufferInfo, KernelAddressInfo, MAX_FRAMEBUFFERS, MAX_MEMORY_REGIONS,
    MAX_MODULES, MemoryRegion, MemoryRegionKind, ModuleInfo,
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
#[unsafe(link_section = ".limine_requests")]
static MODULES: ModulesRequest = ModulesRequest::new();
#[used]
#[unsafe(link_section = ".limine_requests")]
static DATE_AT_BOOT: DateAtBootRequest = DateAtBootRequest::new();
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
    DATE_AT_BOOT.response().unwrap();
}

fn load_boot_info() -> BootInfo {
    let loader = BOOTLOADER.response().unwrap();
    let address = EXECUTABLE_ADDRESS.response().unwrap();
    let unix_seconds_at_boot = u64::try_from(DATE_AT_BOOT.response().unwrap().timestamp)
        .expect("Limine returned a negative boot timestamp");

    BootInfo {
        memory_regions: memory_regions(),
        framebuffers: framebuffers(),
        modules: modules(),
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
        unix_seconds_at_boot,
    }
}

fn modules() -> Vec<ModuleInfo, MAX_MODULES> {
    MODULES
        .response()
        .unwrap()
        .modules()
        .iter()
        .map(|module| ModuleInfo {
            command_line: copy_string(module.cmdline()),
            data: module.data(),
        })
        .collect()
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
    let Some(response) = FRAMEBUFFER.response() else {
        return Vec::new();
    };

    response
        .framebuffers()
        .iter()
        .map(|framebuffer| FramebufferInfo {
            address: framebuffer.address() as u64,
            width: framebuffer.width,
            height: framebuffer.height,
            pitch: framebuffer.pitch,
            bits_per_pixel: framebuffer.bpp,
            memory_model: framebuffer.memory_model,
            red_mask_size: framebuffer.red_mask_size,
            red_mask_shift: framebuffer.red_mask_shift,
            green_mask_size: framebuffer.green_mask_size,
            green_mask_shift: framebuffer.green_mask_shift,
            blue_mask_size: framebuffer.blue_mask_size,
            blue_mask_shift: framebuffer.blue_mask_shift,
        })
        .collect()
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

#[cfg(feature = "kernel-test")]
mod tests {
    use super::{MemoryRegionKind, map_memory_kind, memmap};

    roxy_test::kernel_test!(
        "roxy-boot::maps-limine-memory-kinds",
        maps_limine_memory_kinds,
        {
            let known_kinds = [
                (memmap::MEMMAP_USABLE, MemoryRegionKind::Usable),
                (memmap::MEMMAP_RESERVED, MemoryRegionKind::Reserved),
                (
                    memmap::MEMMAP_ACPI_RECLAIMABLE,
                    MemoryRegionKind::AcpiReclaimable,
                ),
                (memmap::MEMMAP_ACPI_NVS, MemoryRegionKind::AcpiNvs),
                (memmap::MEMMAP_BAD_MEMORY, MemoryRegionKind::BadMemory),
                (
                    memmap::MEMMAP_BOOTLOADER_RECLAIMABLE,
                    MemoryRegionKind::BootloaderReclaimable,
                ),
                (
                    memmap::MEMMAP_EXECUTABLE_AND_MODULES,
                    MemoryRegionKind::ExecutableAndModules,
                ),
                (memmap::MEMMAP_FRAMEBUFFER, MemoryRegionKind::Framebuffer),
                (
                    memmap::MEMMAP_MAPPED_RESERVED,
                    MemoryRegionKind::MappedReserved,
                ),
            ];

            for (limine_kind, expected) in known_kinds {
                assert_eq!(map_memory_kind(limine_kind), expected);
            }

            assert_eq!(map_memory_kind(42), MemoryRegionKind::Unknown(42));
        }
    );
}
