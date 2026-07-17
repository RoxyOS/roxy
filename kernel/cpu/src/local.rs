use roxy_arch::{Architecture, CpuId, CurrentArchitectureBackend};
use spin::Once;

pub struct CpuLocal<T> {
    bsp: Once<T>,
}

impl<T> CpuLocal<T> {
    #[must_use]
    pub const fn new() -> Self {
        Self { bsp: Once::new() }
    }

    /// Initializes the slot belonging to the current CPU.
    ///
    /// # Panics
    ///
    /// Panics outside the BSP or when the slot was already initialized.
    pub fn initialize_current(&self, value: T) {
        assert_eq!(CurrentArchitectureBackend::current_cpu_id(), CpuId::BSP);
        assert!(
            !self.bsp.is_completed(),
            "CPU-local value initialized twice"
        );
        self.bsp.call_once(|| value);
    }

    /// Returns the value belonging to the current CPU.
    ///
    /// # Panics
    ///
    /// Panics outside the BSP or before the slot is initialized.
    #[must_use]
    pub fn get(&self) -> &T {
        assert_eq!(CurrentArchitectureBackend::current_cpu_id(), CpuId::BSP);
        self.bsp.get().unwrap()
    }
}

impl<T> Default for CpuLocal<T> {
    fn default() -> Self {
        Self::new()
    }
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
