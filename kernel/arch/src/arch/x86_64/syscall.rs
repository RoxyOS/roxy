use core::{
    arch::naked_asm,
    cell::UnsafeCell,
    mem::{MaybeUninit, offset_of, size_of, transmute},
    sync::atomic::{AtomicUsize, Ordering},
};

use x86_64::{
    VirtAddr,
    registers::{
        model_specific::{Efer, EferFlags, GsBase, LStar, SFMask, Star},
        rflags::RFlags,
    },
    structures::tss::TaskStateSegment,
};

use crate::{
    Architecture, CurrentArchitectureBackend, MAX_CPUS, RawSyscall, SyscallExit, SyscallHandler,
};

use super::{PerCpuStorage, cpu_map, float, init};

static HANDLER: AtomicUsize = AtomicUsize::new(0);

/// Per-CPU state the naked syscall `entry` needs to cross the privilege boundary, plus the TSS
/// pointer Rust keeps in sync with it.
///
/// Each CPU owns one slot. Every CPU sets its `IA32_GS_BASE` (`GS.base`) to the address of its own
/// slot during bring-up, and `CR4.FSGSBASE` stays clear, so userspace cannot execute `wrgsbase` /
/// `rdgsbase`. The naked `entry` therefore reaches this state directly through `gs:` operand
/// segments, with no `swapgs` exchange required.
///
/// # Safety
///
/// The kernel reserves `GS` entirely for this per-CPU area. Userspace must never load a data
/// segment selector into `GS`, since loading any flat 64-bit data segment zeroes the segment base
/// (the base is forced to zero by long mode for data), which would redirect the next syscall
/// `entry`'s `gs:` reads to address zero. The supported userspaces (mlibc/Bash) never touch `GS`.
#[repr(C)]
pub(super) struct SyscallEntryState {
    /// Kernel stack the syscall `entry` switches onto; this is the current thread's kernel stack on
    /// this CPU, kept in sync with the TSS `RSP0` used by the interrupt-from-user path.
    pub(super) kernel_stack_top: u64,
    /// Saved user `RSP` handed from the syscall prologue to its `EntryFrame` construction.
    pub(super) user_stack_pointer: u64,
    /// Address of this CPU's loaded TSS, so Rust can update its `RSP0` on any CPU.
    tss_pointer: u64,
}

pub(super) const PER_CPU_KERNEL_STACK_TOP: usize = offset_of!(SyscallEntryState, kernel_stack_top);
pub(super) const PER_CPU_USER_STACK_POINTER: usize =
    offset_of!(SyscallEntryState, user_stack_pointer);

static SYSCALL_ENTRY_STATES: PerCpuStorage<SyscallEntryState> =
    PerCpuStorage(UnsafeCell::new([const { MaybeUninit::uninit() }; MAX_CPUS]));

/// Writes and publishes the per-CPU syscall entry state and sets this CPU's `GS` base to it.
///
/// Called once per active CPU during bring-up (the BSP in `init::initialize`, each AP in
/// `init::initialize_ap`, which supplies the per-CPU TSS address), before interrupts are enabled
/// on that CPU.
pub(super) fn initialize_syscall_entry_state(index: usize, tss_pointer: u64) {
    let slot = unsafe { &mut (*SYSCALL_ENTRY_STATES.0.get())[index] };
    // SAFETY: each CPU's slot is uninitialised and written exactly once during its own bring-up.
    slot.write(SyscallEntryState {
        kernel_stack_top: 0,
        user_stack_pointer: 0,
        tss_pointer,
    });

    let slot = unsafe { &mut (*SYSCALL_ENTRY_STATES.0.get())[index] };
    let base = slot.as_mut_ptr().cast::<u8>() as u64;
    GsBase::write(VirtAddr::new(base));
}

/// Returns the per-CPU syscall entry state belonging to `index`. The caller must run on that CPU
/// or hold exclusive access to its brings-up/teardown.
pub(super) fn syscall_entry_state(index: usize) -> &'static mut SyscallEntryState {
    let slot = unsafe { &mut (*SYSCALL_ENTRY_STATES.0.get())[index] };
    // SAFETY: each CPU's slot is initialized in bring-up before it is ever used.
    unsafe { slot.assume_init_mut() }
}

/// Returns the per-CPU syscall entry state belonging to the current CPU.
pub(super) fn current_syscall_entry_state() -> &'static mut SyscallEntryState {
    syscall_entry_state(cpu_map::current_id().get() as usize)
}

/// Returns the TSS loaded on the current CPU, for updating its `RSP0` on any CPU.
pub(super) fn current_cpu_tss() -> *mut TaskStateSegment {
    current_syscall_entry_state().tss_pointer as *mut TaskStateSegment
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct X86_64UserContext {
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub rbp: u64,
    pub rbx: u64,
    pub rax: u64,
    pub rdi: u64,
    pub rsi: u64,
    pub rdx: u64,
    pub r10: u64,
    pub r8: u64,
    pub r9: u64,
    pub instruction_pointer: u64,
    pub flags: u64,
    pub stack_pointer: u64,
    pub fs_base: u64,
}

impl X86_64UserContext {
    #[must_use]
    pub const fn with_syscall_result(mut self, result: u64) -> Self {
        self.rax = result;
        self
    }
}

#[repr(C)]
struct EntryFrame {
    r15: u64,
    r14: u64,
    r13: u64,
    r12: u64,
    rbp: u64,
    rbx: u64,
    rax: u64,
    rdi: u64,
    rsi: u64,
    rdx: u64,
    r10: u64,
    r8: u64,
    r9: u64,
    user_instruction_pointer: u64,
    user_flags: u64,
    user_stack_pointer: u64,
}

const _: () = {
    assert!(offset_of!(EntryFrame, user_instruction_pointer) == 104);
    assert!(offset_of!(EntryFrame, user_flags) == 112);
    assert!(offset_of!(EntryFrame, user_stack_pointer) == 120);
    assert!(size_of::<EntryFrame>() == 128);
};

pub(super) fn configure(handler: SyscallHandler) {
    assert_eq!(
        HANDLER.swap(handler as usize, Ordering::AcqRel),
        0,
        "syscall initialized twice"
    );

    // The BSP is the first CPU running kernel code; program its syscall MSRs. Every AP programs
    // its own copies during AP bring-up in `init::initialize_ap`.
    configure_cpu();
}

/// Programs this CPU's syscall MSRs (`EFER.SCE`, `IA32_STAR`/`LSTAR`/`SFMASK`).
///
/// These MSRs are per logical processor, so this runs once for every active CPU: the BSP via
/// [`configure`] and each AP via `init::initialize_ap`. The values are identical on every CPU
/// (the selectors come from the shared GDT layout and the handler is one permanent entry point),
/// but each processor must write them to its own copies or `syscall`/`sysret` fault on it.
pub(super) fn configure_cpu() {
    let (user_code, user_data, kernel_code, kernel_data) = init::syscall_selectors();

    // SAFETY: architecture initialization established long mode and a permanent entry point.
    unsafe { Efer::update(|flags| flags.insert(EferFlags::SYSTEM_CALL_EXTENSIONS)) };

    Star::write(user_code, user_data, kernel_code, kernel_data)
        .expect("invalid syscall segment layout");
    LStar::write(VirtAddr::new(entry as *const () as u64));
    SFMask::write(RFlags::INTERRUPT_FLAG);
}

pub(super) fn set_kernel_stack_top(kernel_stack_top: u64) {
    current_syscall_entry_state().kernel_stack_top = kernel_stack_top;
}

#[cfg(feature = "kernel-test")]
pub(super) fn kernel_stack_top() -> u64 {
    current_syscall_entry_state().kernel_stack_top
}

#[unsafe(naked)]
unsafe extern "C" fn entry() -> ! {
    naked_asm!(
        "mov qword ptr gs:[{user_stack_pointer}], rsp",
        "mov rsp, qword ptr gs:[{kernel_stack_top}]",
        "push qword ptr gs:[{user_stack_pointer}]",
        "push r11",
        "push rcx",
        "push r9",
        "push r8",
        "push r10",
        "push rdx",
        "push rsi",
        "push rdi",
        "push rax",
        "push rbx",
        "push rbp",
        "push r12",
        "push r13",
        "push r14",
        "push r15",
        "mov rdi, rsp",
        "call {dispatch}",
        "mov rax, [rsp + {rax}]",
        "mov r15, [rsp + {r15}]",
        "mov r14, [rsp + {r14}]",
        "mov r13, [rsp + {r13}]",
        "mov r12, [rsp + {r12}]",
        "mov rbp, [rsp + {rbp}]",
        "mov rbx, [rsp + {rbx}]",
        "mov rdi, [rsp + {rdi}]",
        "mov rsi, [rsp + {rsi}]",
        "mov rdx, [rsp + {rdx}]",
        "mov r10, [rsp + {r10}]",
        "mov r8, [rsp + {r8}]",
        "mov r9, [rsp + {r9}]",
        "mov rcx, [rsp + {user_instruction_pointer}]",
        "mov r11, [rsp + {user_flags}]",
        "mov rsp, [rsp + {saved_user_stack_pointer}]",
        "sysretq",
        kernel_stack_top = const PER_CPU_KERNEL_STACK_TOP,
        user_stack_pointer = const PER_CPU_USER_STACK_POINTER,
        dispatch = sym dispatch,
        rax = const offset_of!(EntryFrame, rax),
        r15 = const offset_of!(EntryFrame, r15),
        r14 = const offset_of!(EntryFrame, r14),
        r13 = const offset_of!(EntryFrame, r13),
        r12 = const offset_of!(EntryFrame, r12),
        rbp = const offset_of!(EntryFrame, rbp),
        rbx = const offset_of!(EntryFrame, rbx),
        rdi = const offset_of!(EntryFrame, rdi),
        rsi = const offset_of!(EntryFrame, rsi),
        rdx = const offset_of!(EntryFrame, rdx),
        r10 = const offset_of!(EntryFrame, r10),
        r8 = const offset_of!(EntryFrame, r8),
        r9 = const offset_of!(EntryFrame, r9),
        user_instruction_pointer = const offset_of!(EntryFrame, user_instruction_pointer),
        user_flags = const offset_of!(EntryFrame, user_flags),
        saved_user_stack_pointer = const offset_of!(EntryFrame, user_stack_pointer),
    )
}

extern "C" fn dispatch(frame: *mut EntryFrame) -> u64 {
    let address = HANDLER.load(Ordering::Acquire);
    assert_ne!(address, 0, "syscall handler not initialized");

    // SAFETY: entry passes a pointer to its complete, live frame on the kernel stack; it stays
    // live until this function returns and the naked epilogue reads it back.
    let frame = unsafe { &mut *frame };

    let request = RawSyscall {
        number: frame.rax,
        arguments: [
            frame.rdi, frame.rsi, frame.rdx, frame.r10, frame.r8, frame.r9,
        ],
        context: X86_64UserContext {
            r15: frame.r15,
            r14: frame.r14,
            r13: frame.r13,
            r12: frame.r12,
            rbp: frame.rbp,
            rbx: frame.rbx,
            rax: frame.rax,
            rdi: frame.rdi,
            rsi: frame.rsi,
            rdx: frame.rdx,
            r10: frame.r10,
            r8: frame.r8,
            r9: frame.r9,
            instruction_pointer: frame.user_instruction_pointer,
            flags: frame.user_flags,
            stack_pointer: frame.user_stack_pointer,
            fs_base: CurrentArchitectureBackend::user_thread_pointer(),
        },
    };

    // SAFETY: configure stores one permanent SyscallHandler function pointer.
    let handler: SyscallHandler = unsafe { transmute(address) };

    // The naked epilogue restores rax from the frame, so every branch must write it there.
    match handler(request) {
        SyscallExit::Returned(value) => frame.rax = value,
        SyscallExit::Resume {
            return_value,
            resume,
        } => {
            frame.rax = return_value;
            frame.user_instruction_pointer = resume.instruction_pointer;
            frame.user_stack_pointer = resume.stack_pointer;
            frame.rdi = resume.arguments[0];
            frame.rsi = resume.arguments[1];
            frame.rdx = resume.arguments[2];
        }
        SyscallExit::RestoreContext(restore) => {
            *frame = EntryFrame {
                r15: restore.r15,
                r14: restore.r14,
                r13: restore.r13,
                r12: restore.r12,
                rbp: restore.rbp,
                rbx: restore.rbx,
                rax: restore.rax,
                rdi: restore.rdi,
                rsi: restore.rsi,
                rdx: restore.rdx,
                r10: restore.r10,
                r8: restore.r8,
                r9: restore.r9,
                user_instruction_pointer: restore.instruction_pointer,
                user_flags: restore.flags,
                user_stack_pointer: restore.stack_pointer,
            };
        }
    }

    0
}

pub(super) unsafe fn resume_user(instruction_pointer: u64, stack_pointer: u64) -> ! {
    CurrentArchitectureBackend::set_user_thread_pointer(0);
    float::reset();

    // SAFETY: the caller guarantees valid user mappings and this function resets user state.
    unsafe { sysret_fresh(instruction_pointer, stack_pointer) }
}

#[unsafe(naked)]
unsafe extern "C" fn sysret_fresh(_instruction_pointer: u64, _stack_pointer: u64) -> ! {
    naked_asm!(
        "mov rcx, rdi",
        "mov rsp, rsi",
        "mov r11, 0x202",
        "xor r15, r15",
        "xor r14, r14",
        "xor r13, r13",
        "xor r12, r12",
        "xor rbp, rbp",
        "xor rbx, rbx",
        "xor rax, rax",
        "xor rdi, rdi",
        "xor rsi, rsi",
        "xor rdx, rdx",
        "xor r10, r10",
        "xor r8, r8",
        "xor r9, r9",
        "sysretq",
    )
}
