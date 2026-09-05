use roxy_arch::{Architecture, CurrentArchitectureBackend, MAX_CPUS};
use spin::Once;

/// Per-CPU storage: one `spin::Once<T>` slot per possible CPU, indexed by the current
/// architecture CPU id.
///
/// Unlike a BSP-only singleton, every CPU has its own slot and there is no assertion that the
/// caller runs on the bootstrap processor. `initialize_current` writes the current CPU's slot
/// exactly once and `get` reads the current CPU's slot after initialization. `spin::Once` carries
/// the write-before-publish ordering (release on completion, acquire on read), so `get` never
/// observes a partially written value.
///
/// The struct is automatically `Sync` when `spin::Once<T>` is, i.e. when `T: Send + Sync`; no
/// custom `unsafe impl` is required.
pub struct CpuLocal<T> {
    slots: [Once<T>; MAX_CPUS],
}

impl<T> CpuLocal<T> {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            slots: [const { Once::new() }; MAX_CPUS],
        }
    }

    /// Initializes the slot belonging to the current CPU.
    ///
    /// # Panics
    ///
    /// Panics when the current CPU's slot was already initialized.
    pub fn initialize_current(&self, value: T) {
        let index = current_cpu_index();
        assert!(
            !self.slots[index].is_completed(),
            "CPU-local value initialized twice"
        );
        self.slots[index].call_once(|| value);
    }

    /// Returns the value belonging to the current CPU.
    ///
    /// # Panics
    ///
    /// Panics when the current CPU's slot has not been initialized.
    #[must_use]
    pub fn get(&self) -> &T {
        let index = current_cpu_index();
        self.slots[index]
            .get()
            .expect("CPU-local value accessed before initialization")
    }
}

impl<T> Default for CpuLocal<T> {
    fn default() -> Self {
        Self::new()
    }
}

fn current_cpu_index() -> usize {
    let id = CurrentArchitectureBackend::current_cpu_id().get() as usize;
    assert!(id < MAX_CPUS, "CPU id {id} out of bounds ({MAX_CPUS})");
    id
}

#[cfg(feature = "kernel-test")]
mod tests {
    use super::CpuLocal;

    roxy_test::kernel_test!(
        "roxy-cpu::cpu-local-stores-current-value",
        cpu_local_stores_current_value,
        {
            let local = CpuLocal::new();

            local.initialize_current(0x5a_u8);

            assert_eq!(*local.get(), 0x5a);
        }
    );
}
