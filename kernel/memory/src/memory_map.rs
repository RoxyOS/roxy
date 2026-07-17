use alloc::vec::Vec;

use roxy_boot::{BootInfo, MemoryRegionKind};

use crate::{
    PhysicalAddress, VirtualAddress,
    address::{PAGE_SIZE, align_down, align_up},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MemoryRegion {
    pub start: u64,
    pub end: u64,
    pub kind: MemoryRegionKind,
}

pub(crate) struct MemoryMap {
    pub regions: Vec<MemoryRegion>,
}

impl MemoryMap {
    pub fn from_boot_info(boot_info: &BootInfo) -> Self {
        let source = collect_regions(boot_info);
        let boundaries = collect_boundaries(&source);
        let mut regions = Vec::new();

        for window in boundaries.windows(2) {
            insert_region(&mut regions, &source, window[0], window[1]);
        }

        validate_reserved_regions(boot_info, &regions);
        Self { regions }
    }
}

fn collect_regions(boot_info: &BootInfo) -> Vec<MemoryRegion> {
    boot_info
        .memory_regions
        .iter()
        .filter_map(|region| {
            let end = region.base.checked_add(region.length).unwrap();
            let usable = region.kind == MemoryRegionKind::Usable;
            let start = if usable {
                align_up(region.base).unwrap()
            } else {
                align_down(region.base)
            };
            let end = if usable {
                align_down(end)
            } else {
                align_up(end).unwrap()
            };

            (start < end).then_some(MemoryRegion {
                start,
                end,
                kind: region.kind,
            })
        })
        .collect()
}

fn collect_boundaries(regions: &[MemoryRegion]) -> Vec<u64> {
    let mut boundaries: Vec<u64> = regions
        .iter()
        .flat_map(|region| [region.start, region.end])
        .collect();
    boundaries.sort_unstable();
    boundaries.dedup();
    boundaries
}

fn insert_region(output: &mut Vec<MemoryRegion>, source: &[MemoryRegion], start: u64, end: u64) {
    let Some(kind) = region_kind_at(source, start, end) else {
        return;
    };

    if let Some(previous) = output.last_mut()
        && previous.end == start
        && previous.kind == kind
    {
        previous.end = end;
        return;
    }

    output.push(MemoryRegion { start, end, kind });
}

fn region_kind_at(source: &[MemoryRegion], start: u64, end: u64) -> Option<MemoryRegionKind> {
    let mut covering = source
        .iter()
        .filter(|region| region.start < end && region.end > start)
        .map(|region| region.kind);
    let first = covering.next()?;

    Some(
        covering
            .find(|kind| *kind != MemoryRegionKind::Usable)
            .unwrap_or(first),
    )
}

fn validate_reserved_regions(boot_info: &BootInfo, regions: &[MemoryRegion]) {
    let kernel = PhysicalAddress::new(boot_info.kernel_address.physical_base).unwrap();
    assert_address_reserved(regions, kernel);

    let rsdp = VirtualAddress::new(boot_info.rsdp_address).unwrap();
    assert_address_reserved(regions, hhdm_to_physical(rsdp, boot_info.hhdm_offset));

    for framebuffer in &boot_info.framebuffers {
        let start = VirtualAddress::new(framebuffer.address).unwrap();
        let end = framebuffer
            .address
            .checked_add(framebuffer.pitch.checked_mul(framebuffer.height).unwrap())
            .unwrap();
        let end = VirtualAddress::new(end).unwrap();
        let start = hhdm_to_physical(start, boot_info.hhdm_offset);
        let end = hhdm_to_physical(end, boot_info.hhdm_offset);
        let last = PhysicalAddress::new(end.as_u64().checked_sub(1).unwrap()).unwrap();

        assert_address_reserved(regions, start);
        assert_address_reserved(regions, last);
    }
}

fn hhdm_to_physical(address: VirtualAddress, hhdm_offset: u64) -> PhysicalAddress {
    let address = address.as_u64().checked_sub(hhdm_offset).unwrap();
    PhysicalAddress::new(address).unwrap()
}

fn assert_address_reserved(regions: &[MemoryRegion], address: PhysicalAddress) {
    let address = address.as_u64();
    let region = regions
        .iter()
        .find(|region| region.start <= address && address < region.end)
        .unwrap();
    assert_ne!(region.kind, MemoryRegionKind::Usable);
    assert_eq!(region.start % PAGE_SIZE, 0);
    assert_eq!(region.end % PAGE_SIZE, 0);
}
