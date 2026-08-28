//! The `x86_64` signal-frame layout and `sigreturn` trampoline.

use core::{mem::size_of, ptr, slice};

use roxy_arch::UserContext;
use roxy_signal::SignalSet;

use super::SIGRETURN_SYSCALL_NUMBER;

/// Base address of the one-page read-execute trampoline mapping.
///
/// Sits between the interpreter region and the user stack in the current process layout.
pub(crate) const TRAMPOLINE_BASE: u64 = 0x0000_4000_0000_0000;

const RETURN_ADDRESS_SIZE: usize = size_of::<u64>();
const OLD_MASK_SIZE: usize = size_of::<u64>();
pub(crate) const USER_CONTEXT_SIZE: usize = size_of::<UserContext>();

/// Total frame size: return address, saved user context, and the mask active before delivery.
pub(crate) const SIGNAL_FRAME_SIZE: usize = RETURN_ADDRESS_SIZE + USER_CONTEXT_SIZE + OLD_MASK_SIZE;

pub(crate) const USER_CONTEXT_OFFSET: usize = RETURN_ADDRESS_SIZE;
const OLD_MASK_OFFSET: usize = USER_CONTEXT_OFFSET + USER_CONTEXT_SIZE;

core::arch::global_asm!(
    ".section .rodata",
    ".globl {trampoline}",
    "{trampoline}:",
    "mov eax, {number}",
    "syscall",
    ".globl {trampoline_end}",
    "{trampoline_end}:",
    trampoline = sym ROXY_SIGRETURN_TRAMPOLINE,
    trampoline_end = sym ROXY_SIGRETURN_TRAMPOLINE_END,
    number = const SIGRETURN_SYSCALL_NUMBER,
);

unsafe extern "C" {
    static ROXY_SIGRETURN_TRAMPOLINE: u8;
    static ROXY_SIGRETURN_TRAMPOLINE_END: u8;
}

/// Returns the assembled trampoline bytes to copy into a user mapping.
///
/// The bytes live in kernel `.rodata` and are never executed in kernel mode; the user page gets
/// its own copy so no kernel memory is shared with userspace.
#[must_use]
pub(crate) fn trampoline() -> &'static [u8] {
    let start = ptr::addr_of!(ROXY_SIGRETURN_TRAMPOLINE);
    let end = ptr::addr_of!(ROXY_SIGRETURN_TRAMPOLINE_END);

    // SAFETY: the global assembly places both symbols adjacently in `.rodata`, so the byte range
    // between them is initialized and valid for reads for the lifetime of the program.
    unsafe { slice::from_raw_parts(start, end as usize - start as usize) }
}

/// Builds the frame bytes for one signal delivery.
///
/// Layout: the trampoline entry (the handler's `ret` target), a snapshot of the interrupted user
/// context that `sigreturn` restores, and the mask that was active before delivery.
///
/// The frame address must satisfy the System V entry alignment (`frame % 16 == 8`), which the
/// caller selects when placing the frame.
#[must_use]
pub(crate) fn build_bytes(context: &UserContext, old_mask: SignalSet) -> [u8; SIGNAL_FRAME_SIZE] {
    let mut frame = [0u8; SIGNAL_FRAME_SIZE];

    // The frame's return address targets the user-mapped trampoline copy at `TRAMPOLINE_BASE`,
    // not the kernel rodata source that `trampoline()` hands out for copying.
    write_u64(&mut frame[0..RETURN_ADDRESS_SIZE], TRAMPOLINE_BASE);
    write_u64(
        &mut frame[OLD_MASK_OFFSET..OLD_MASK_OFFSET + OLD_MASK_SIZE],
        old_mask.bits(),
    );
    write_context(
        &mut frame[USER_CONTEXT_OFFSET..USER_CONTEXT_OFFSET + USER_CONTEXT_SIZE],
        context,
    );

    frame
}

/// Restores the interrupted user context from a frame's context snapshot.
///
/// # Panics
///
/// Panics when `bytes` does not have the exact frame snapshot size; the caller only passes
/// kernel-recorded frames.
#[must_use]
pub(crate) fn restore_context(bytes: &[u8]) -> UserContext {
    assert_eq!(bytes.len(), USER_CONTEXT_SIZE, "signal frame size mismatch");

    UserContext {
        r15: read_u64(slot(bytes, 0)),
        r14: read_u64(slot(bytes, 1)),
        r13: read_u64(slot(bytes, 2)),
        r12: read_u64(slot(bytes, 3)),
        rbp: read_u64(slot(bytes, 4)),
        rbx: read_u64(slot(bytes, 5)),
        rax: read_u64(slot(bytes, 6)),
        rdi: read_u64(slot(bytes, 7)),
        rsi: read_u64(slot(bytes, 8)),
        rdx: read_u64(slot(bytes, 9)),
        r10: read_u64(slot(bytes, 10)),
        r8: read_u64(slot(bytes, 11)),
        r9: read_u64(slot(bytes, 12)),
        instruction_pointer: read_u64(slot(bytes, 13)),
        flags: read_u64(slot(bytes, 14)),
        stack_pointer: read_u64(slot(bytes, 15)),
        fs_base: read_u64(slot(bytes, 16)),
    }
}

/// Restores the pre-delivery signal mask from a frame's mask slot.
#[must_use]
pub(crate) fn restore_old_mask(bytes: &[u8]) -> SignalSet {
    let mask = read_u64(
        bytes[OLD_MASK_OFFSET..OLD_MASK_OFFSET + OLD_MASK_SIZE]
            .try_into()
            .expect("mask slot size"),
    );

    SignalSet::from_bits_retain(mask)
}

/// Returns the eight-byte slot at `index` within the context snapshot.
fn slot(bytes: &[u8], index: usize) -> [u8; 8] {
    let start = index * size_of::<u64>();

    bytes[start..start + size_of::<u64>()]
        .try_into()
        .expect("register slot size")
}

fn write_u64(slot: &mut [u8], value: u64) {
    slot.copy_from_slice(&value.to_le_bytes());
}

fn read_u64(slot: [u8; 8]) -> u64 {
    u64::from_le_bytes(slot)
}

fn write_context(slot: &mut [u8], context: &UserContext) {
    let fields = [
        context.r15,
        context.r14,
        context.r13,
        context.r12,
        context.rbp,
        context.rbx,
        context.rax,
        context.rdi,
        context.rsi,
        context.rdx,
        context.r10,
        context.r8,
        context.r9,
        context.instruction_pointer,
        context.flags,
        context.stack_pointer,
        context.fs_base,
    ];

    for (index, value) in fields.iter().enumerate() {
        let start = index * size_of::<u64>();
        write_u64(&mut slot[start..start + size_of::<u64>()], *value);
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_arch::UserContext;
    use roxy_signal::SignalSet;
    use roxy_test::kernel_test;

    use super::{
        RETURN_ADDRESS_SIZE, SIGRETURN_SYSCALL_NUMBER, TRAMPOLINE_BASE, USER_CONTEXT_OFFSET,
        build_bytes, restore_context, restore_old_mask, trampoline,
    };

    fn sample_context() -> UserContext {
        UserContext {
            r15: 0x1000,
            r14: 0x1001,
            r13: 0x1002,
            r12: 0x1003,
            rbp: 0x1004,
            rbx: 0x1005,
            rax: 0x1006,
            rdi: 0x1007,
            rsi: 0x1008,
            rdx: 0x1009,
            r10: 0x100a,
            r8: 0x100b,
            r9: 0x100c,
            instruction_pointer: 0x2000,
            flags: 0x202,
            stack_pointer: 0x7fff_ffff_f000,
            fs_base: 0x3000,
        }
    }

    kernel_test!("roxy-signal::frame", round_trips_context_and_mask, {
        let context = sample_context();
        let old_mask = SignalSet::USER1 | SignalSet::ALARM;
        let frame = build_bytes(&context, old_mask);

        assert_eq!(restore_context(&frame[USER_CONTEXT_OFFSET..]), context);
        assert_eq!(restore_old_mask(&frame), old_mask);
    });

    kernel_test!("roxy-signal::frame", aims_return_address_at_trampoline, {
        let frame = build_bytes(&sample_context(), SignalSet::empty());

        assert_eq!(
            u64::from_le_bytes(
                frame[0..RETURN_ADDRESS_SIZE]
                    .try_into()
                    .expect("return address size")
            ),
            TRAMPOLINE_BASE
        );
    });

    kernel_test!("roxy-signal::frame", trampoline_issues_sigreturn, {
        let trampoline = trampoline();

        // mov eax, imm32 with the sigreturn number encoded little-endian, followed by syscall.
        assert_eq!(trampoline[0], 0xb8);
        assert_eq!(
            u32::from_le_bytes(trampoline[1..5].try_into().expect("immediate size")),
            u32::try_from(SIGRETURN_SYSCALL_NUMBER).expect("syscall number fits u32")
        );
        assert_eq!(&trampoline[5..], &[0x0f, 0x05]);
    });
}
