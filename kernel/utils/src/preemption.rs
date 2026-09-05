use core::{cell::UnsafeCell, marker::PhantomData};

use roxy_arch::{Architecture, CpuId, CurrentArchitectureBackend, MAX_CPUS};

/// Per-CPU preemption depth. Each CPU exclusively accesses its own slot (indexed by its logical
/// `CpuId`), so no atomic or mutex is needed for mutual exclusion.
struct PerCpuDepth(UnsafeCell<[u32; MAX_CPUS]>);

// SAFETY: `current_depth` grants a `&mut u32` to the current CPU's slot only; no other CPU reads
// or writes that slot, so the `UnsafeCell` interior-mutability invariant is upheld at runtime.
unsafe impl Sync for PerCpuDepth {}

static PREEMPTION_DEPTH: PerCpuDepth = PerCpuDepth(UnsafeCell::new([0; MAX_CPUS]));

/// RAII guard that prevents the current CPU from switching threads.
pub struct PreemptionGuard {
    cpu_id: CpuId,
    not_send: PhantomData<*mut ()>,
}

/// Calls `f` with a mutable reference to the current CPU's preemption depth.
///
/// The reference is valid only for the duration of the call and is guaranteed to belong
/// exclusively to the calling CPU.
fn with_depth<T>(f: impl FnOnce(&mut u32) -> T) -> T {
    let cpu_id = CurrentArchitectureBackend::current_cpu_id();
    let index = cpu_id.get() as usize;
    assert!(
        index < MAX_CPUS,
        "CPU id {index} exceeds MAX_CPUS ({MAX_CPUS})"
    );

    // SAFETY: Each CPU writes to and reads from its own slot exclusively. No other CPU reads or
    // writes this slot, so the `&mut` aliasing rules are satisfied at runtime.
    f(unsafe { &mut (*PREEMPTION_DEPTH.0.get())[index] })
}

#[must_use]
/// Disables thread preemption on the current CPU until the returned guard is dropped.
///
/// # Panics
///
/// Panics when the nesting depth overflows.
pub fn disable() -> PreemptionGuard {
    let cpu_id = CurrentArchitectureBackend::current_cpu_id();
    with_depth(|depth| {
        *depth = depth.checked_add(1).expect("preemption depth overflow");
    });
    PreemptionGuard {
        cpu_id,
        not_send: PhantomData,
    }
}

#[must_use]
/// Reports whether thread preemption is disabled on the current CPU.
pub fn is_disabled() -> bool {
    with_depth(|depth| *depth != 0)
}

impl Drop for PreemptionGuard {
    fn drop(&mut self) {
        assert_eq!(self.cpu_id, CurrentArchitectureBackend::current_cpu_id());
        with_depth(|depth| {
            *depth = depth.checked_sub(1).expect("unbalanced preemption enable");
        });
    }
}
