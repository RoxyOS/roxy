use core::{
    marker::PhantomData,
    sync::atomic::{AtomicU32, Ordering},
};

use roxy_arch::{Architecture, CpuId, CurrentArchitectureBackend};

static BSP_DEPTH: AtomicU32 = AtomicU32::new(0);

/// RAII guard that prevents the current CPU from switching threads.
pub struct PreemptionGuard {
    cpu_id: CpuId,
    not_send: PhantomData<*mut ()>,
}

#[must_use]
/// Disables thread preemption on the current CPU until the returned guard is dropped.
///
/// # Panics
///
/// Panics outside the initialized BSP or when the nesting depth overflows.
pub fn disable() -> PreemptionGuard {
    let cpu_id = CurrentArchitectureBackend::current_cpu_id();
    assert_eq!(cpu_id, CpuId::BSP);
    BSP_DEPTH
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |depth| {
            depth.checked_add(1)
        })
        .expect("preemption depth overflow");
    PreemptionGuard {
        cpu_id,
        not_send: PhantomData,
    }
}

#[must_use]
/// Reports whether thread preemption is disabled on the current CPU.
///
/// # Panics
///
/// Panics outside the initialized BSP.
pub fn is_disabled() -> bool {
    assert_eq!(CurrentArchitectureBackend::current_cpu_id(), CpuId::BSP);
    BSP_DEPTH.load(Ordering::Relaxed) != 0
}

impl Drop for PreemptionGuard {
    fn drop(&mut self) {
        assert_eq!(self.cpu_id, CurrentArchitectureBackend::current_cpu_id());
        BSP_DEPTH
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |depth| {
                depth.checked_sub(1)
            })
            .expect("unbalanced preemption enable");
    }
}
