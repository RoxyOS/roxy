use spin::Mutex;

use crate::{CpuId, MAX_CPUS};

/// Maps the hardware APIC id of each CPU to the kernel's logical `CpuId` slot.
///
/// The `x86_64` backend identifies "which CPU am I" by the current `APIC` id and maps it to a
/// compact, densely numbered logical slot (`0..n-1`, starting with the BSP as slot 0) that the
/// rest of the kernel uses to index per-CPU storage such as `CpuLocal`. Holding the two ids apart
/// matters: APIC ids are sparse hardware numbers and have no reason to be valid array indices,
/// while `CpuId` must index `[T; MAX_CPUS]` arrays safely.
#[derive(Clone, Copy)]
struct Entry {
    apic_id: u32,
    slot: CpuId,
}

struct CpuMap {
    entries: [Entry; MAX_CPUS],
    count: usize,
}

static MAP: Mutex<CpuMap> = Mutex::new(CpuMap {
    entries: [Entry {
        // Sentinel for an unoccupied slot; reads never observe it because they are bounded by
        // `count`. It only satisfies the `const` array initializer.
        apic_id: u32::MAX,
        slot: CpuId::BSP,
    }; MAX_CPUS],
    count: 0,
});

/// Registers `apic_id`, assigning the next free logical slot. Registration is a side effect;
/// the mapped slot is queried with `current_id`.
///
/// # Panics
///
/// Panics when `apic_id` is already registered (a CPU must register exactly once) or when more
/// than `MAX_CPUS` distinct CPUs register.
pub(super) fn register(apic_id: u32) {
    let mut map = MAP.lock();
    for &entry in &map.entries[..map.count] {
        assert!(
            entry.apic_id != apic_id,
            "CPU with apic id {apic_id} is already registered"
        );
    }
    assert!(map.count < MAX_CPUS, "CPU map capacity exceeded (MAX_CPUS)");
    let index = map.count;
    // `index` is bounded by `MAX_CPUS`, so the narrow conversion always succeeds.
    let slot = CpuId::new(u32::try_from(index).expect("MAX_CPUS fits in u32"));
    map.entries[index] = Entry { apic_id, slot };
    map.count += 1;
}

/// Returns the logical slot of the CPU currently executing.
///
/// # Panics
///
/// Panics when the current CPU has not been registered by its bring-up path.
pub(super) fn current_id() -> CpuId {
    let apic_id = read_current_apic_id();
    let map = MAP.lock();
    map.entries[..map.count]
        .iter()
        .find(|entry| entry.apic_id == apic_id)
        .map_or_else(
            || panic!("CPU with apic id {apic_id} is not registered"),
            |entry| entry.slot,
        )
}

/// Reads the current CPU's hardware APIC id via CPUID leaf 1 (`EBX[31:24]`, Initial APIC ID).
///
/// CPUID is used instead of the x2APIC ID MSR so this works before x2APIC mode is enabled.
pub(super) fn read_current_apic_id() -> u32 {
    let result = core::arch::x86_64::__cpuid(1);
    result.ebx >> 24
}

#[cfg(feature = "kernel-test")]
mod tests {
    use crate::CpuId;

    use super::current_id;

    roxy_test::kernel_test!(
        "roxy-arch::cpu-map-current-cpu-resolves-to-bsp",
        cpu_map_current_cpu_resolves_to_bsp,
        {
            // The BSP registered during boot; the current CPU must resolve to logical slot 0.
            assert_eq!(current_id(), CpuId::BSP);
        }
    );
}
