use core::arch::asm;

use x86_64::registers::control::{Cr0, Cr0Flags, Cr4, Cr4Flags};

const STATE_SIZE: usize = 512;

#[repr(C, align(16))]
pub struct X86_64FloatState {
    bytes: [u8; STATE_SIZE],
}

impl X86_64FloatState {
    #[must_use]
    pub const fn initial() -> Self {
        let mut bytes = [0; STATE_SIZE];
        bytes[0] = 0x7f;
        bytes[1] = 0x03;
        bytes[24] = 0x80;
        bytes[25] = 0x1f;

        Self { bytes }
    }

    /// Captures the current x87, MMX, and SSE state.
    ///
    /// # Safety
    ///
    /// The CPU must have been configured by this module before this operation.
    #[must_use]
    pub unsafe fn capture_current() -> Self {
        let mut state = Self::initial();

        // SAFETY: The state is live, writable, 16-byte aligned, and exactly 512 bytes.
        unsafe { state.save() };

        state
    }

    /// Saves the current x87, MMX, and SSE state into this value.
    ///
    /// # Safety
    ///
    /// The CPU must have been configured by this module before this operation.
    pub unsafe fn save(&mut self) {
        // SAFETY: `bytes` is a live, writable, 16-byte-aligned 512-byte FXSAVE region.
        unsafe {
            asm!(
                "fxsave64 [{}]",
                in(reg) self.bytes.as_mut_ptr(),
                options(nostack, preserves_flags)
            );
        }
    }

    /// Restores this x87, MMX, and SSE state on the current CPU.
    ///
    /// # Safety
    ///
    /// The CPU must have been configured by this module and this value must contain a state image
    /// produced by `initial` or `save`.
    pub unsafe fn restore(&self) {
        // SAFETY: `bytes` is a live, readable, 16-byte-aligned valid FXRSTOR region.
        unsafe {
            asm!(
                "fxrstor64 [{}]",
                in(reg) self.bytes.as_ptr(),
                options(nostack, preserves_flags)
            );
        }
    }
}

pub(super) fn initialize() {
    // SAFETY: x86_64 requires x87 and SSE2. Initialization runs once with interrupts disabled,
    // preserves unrelated control bits, and establishes the state contract before threads exist.
    unsafe {
        Cr0::update(|flags| {
            flags.remove(Cr0Flags::EMULATE_COPROCESSOR | Cr0Flags::TASK_SWITCHED);
            flags.insert(Cr0Flags::MONITOR_COPROCESSOR | Cr0Flags::NUMERIC_ERROR);
        });
        Cr4::update(|flags| {
            flags.insert(Cr4Flags::OSFXSR | Cr4Flags::OSXMMEXCPT_ENABLE);
        });
    }

    reset();
}

pub(super) fn reset() {
    let state = X86_64FloatState::initial();

    // SAFETY: `initialize` configured the CPU before this function is used.
    unsafe { state.restore() };
}

#[cfg(feature = "kernel-test")]
mod tests {
    use x86_64::registers::control::{Cr0, Cr0Flags, Cr4, Cr4Flags};

    roxy_test::kernel_test!("roxy-arch::float-control-state", float_control_state, {
        let cr0 = Cr0::read();
        let cr4 = Cr4::read();

        assert!(cr0.contains(Cr0Flags::MONITOR_COPROCESSOR));
        assert!(cr0.contains(Cr0Flags::NUMERIC_ERROR));
        assert!(!cr0.intersects(Cr0Flags::EMULATE_COPROCESSOR | Cr0Flags::TASK_SWITCHED));
        assert!(cr4.contains(Cr4Flags::OSFXSR));
        assert!(cr4.contains(Cr4Flags::OSXMMEXCPT_ENABLE));
    });
}
