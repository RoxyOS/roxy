use core::arch::asm;

use x86_64::registers::control::{Cr0, Cr0Flags, Cr4, Cr4Flags};
use x86_64::registers::xcontrol::{XCr0, XCr0Flags};

/// Size of the uncompacted XSAVE area with x87, SSE, and AVX state components.
///
/// Layout (uncompacted):
///   - Bytes 0–511:   Legacy FXSAVE region (x87 + SSE)
///   - Bytes 512–575: XSAVE header (64 bytes; `XSTATE_BV` at offset 512)
///   - Bytes 576–831: `YMM_Hi128` (16 registers × 16 upper bytes = 256 bytes)
///
/// Total minimum: 832 bytes.  We use a 64-byte-aligned safe constant of 1088.
const STATE_SIZE: usize = 1088;

/// Mask for `xsave`/`xrstor`: save/restore x87 (bit 0), SSE (bit 1), and AVX (bit 2).
const XSAVE_STATE_MASK: u32 = 0b111;

#[repr(C, align(64))]
pub struct X86_64FloatState {
    bytes: [u8; STATE_SIZE],
}

impl X86_64FloatState {
    #[must_use]
    pub const fn initial() -> Self {
        let mut bytes = [0; STATE_SIZE];
        // x87 FCW: default 0x037F.
        bytes[0] = 0x7f;
        bytes[1] = 0x03;
        // SSE MXCSR: default 0x1F80.
        bytes[24] = 0x80;
        bytes[25] = 0x1f;
        // XSAVE header XSTATE_BV: report x87 | SSE | AVX as valid.
        // YMM_Hi128 (offset 576) is already zeroed by the `[0; _]` initializer.
        bytes[512] = 0x07;

        Self { bytes }
    }

    /// Captures the current x87, SSE, and AVX state.
    ///
    /// # Safety
    ///
    /// The CPU must have been configured by this module before this operation.
    #[must_use]
    pub unsafe fn capture_current() -> Self {
        let mut state = Self::initial();

        // SAFETY: The state is live, writable, 64-byte aligned, and large enough for the
        // uncompacted XSAVE region with x87, SSE, and AVX components.
        unsafe { state.save() };

        state
    }

    /// Saves the current x87, SSE, and AVX state into this value.
    ///
    /// # Safety
    ///
    /// The CPU must have been configured by this module before this operation.
    pub unsafe fn save(&mut self) {
        // SAFETY: `bytes` is a live, writable, 64-byte-aligned, STATE_SIZE-byte XSAVE region.
        // EAX = low 32 bits of the request mask; EDX = high 32 bits.  Both are set to zero
        // for the high part; XSAVE_STATE_MASK fits in a u32.
        unsafe {
            asm!(
                "xsave64 [{}]",
                in(reg) self.bytes.as_mut_ptr(),
                in("eax") XSAVE_STATE_MASK,
                in("edx") 0u32,
                options(nostack, preserves_flags)
            );
        }
    }

    /// Restores this x87, SSE, and AVX state on the current CPU.
    ///
    /// # Safety
    ///
    /// The CPU must have been configured by this module and this value must contain a state image
    /// produced by `initial` or `save`.
    pub unsafe fn restore(&self) {
        // SAFETY: `bytes` is a live, readable, 64-byte-aligned valid XSAVE region.  The
        // instruction reads XSTATE_BV from the buffer header to determine which components
        // to restore (logical AND of buffer XSTATE_BV and the request mask EAX:EDX).
        unsafe {
            asm!(
                "xrstor64 [{}]",
                in(reg) self.bytes.as_ptr(),
                in("eax") XSAVE_STATE_MASK,
                in("edx") 0u32,
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
            flags.insert(Cr4Flags::OSFXSR | Cr4Flags::OSXMMEXCPT_ENABLE | Cr4Flags::OSXSAVE);
        });
        // Enable x87, SSE, and AVX state components in XCR0 so that XSAVE/XRSTOR manage them.
        XCr0::write(XCr0Flags::X87 | XCr0Flags::SSE | XCr0Flags::AVX);
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
    use x86_64::registers::xcontrol::{XCr0, XCr0Flags};

    roxy_test::kernel_test!("roxy-arch::float-control-state", float_control_state, {
        let cr0 = Cr0::read();
        let cr4 = Cr4::read();

        assert!(cr0.contains(Cr0Flags::MONITOR_COPROCESSOR));
        assert!(cr0.contains(Cr0Flags::NUMERIC_ERROR));
        assert!(!cr0.intersects(Cr0Flags::EMULATE_COPROCESSOR | Cr0Flags::TASK_SWITCHED));
        assert!(cr4.contains(Cr4Flags::OSFXSR));
        assert!(cr4.contains(Cr4Flags::OSXMMEXCPT_ENABLE));
        assert!(cr4.contains(Cr4Flags::OSXSAVE));

        let xcr0 = XCr0::read();
        assert!(
            xcr0.contains(XCr0Flags::X87 | XCr0Flags::SSE | XCr0Flags::AVX),
            "XCR0 must enable x87, SSE, and AVX state components"
        );
    });
}
