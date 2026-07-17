use alloc::vec::Vec;

use roxy_boot::{BootInfo, MemoryRegionKind};

use crate::address::{PAGE_SIZE, align_down, align_up};

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
    assert_address_reserved(regions, boot_info.kernel_address.physical_base);
    assert_address_reserved(regions, boot_info.rsdp_address);

    for framebuffer in &boot_info.framebuffers {
        assert_address_reserved(regions, framebuffer.address);
        let end = framebuffer
            .address
            .checked_add(framebuffer.pitch.checked_mul(framebuffer.height).unwrap())
            .unwrap();
        assert_address_reserved(regions, end.saturating_sub(1));
    }
}

fn assert_address_reserved(regions: &[MemoryRegion], address: u64) {
    let region = regions
        .iter()
        .find(|region| region.start <= address && address < region.end)
        .unwrap();
    assert_ne!(region.kind, MemoryRegionKind::Usable);
    assert_eq!(region.start % PAGE_SIZE, 0);
    assert_eq!(region.end % PAGE_SIZE, 0);
}
