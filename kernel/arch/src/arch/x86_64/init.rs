use core::{
    cell::UnsafeCell,
    mem::{MaybeUninit, offset_of},
};
use spin::Once;
use tap::Tap;
use x86_64::{
    VirtAddr,
    instructions::{
        segmentation::{CS, DS, ES, SS, Segment},
        tables::load_tss,
    },
    registers::model_specific::GsBase,
    structures::{
        gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector},
        idt::InterruptDescriptorTable,
        tss::TaskStateSegment,
    },
};

use crate::{CpuId, ExceptionHandler, MAX_CPUS};

use super::{cpu_map, exception, float, interrupt};

const DOUBLE_FAULT_IST_INDEX: u16 = 0;
const DOUBLE_FAULT_STACK_SIZE: usize = 4096 * 5;
const DOUBLE_FAULT_STACK_OFFSET: u64 = 4096 * 5;
const AP_KERNEL_STACK_SIZE: usize = 4096 * 4;

// ── BSP state ───────────────────────────────────────────────────────────────

static BSP_TSS: Once<MutableTss> = Once::new();
static BSP_GDT: Once<(GlobalDescriptorTable, Selectors)> = Once::new();
static IDT: Once<InterruptDescriptorTable> = Once::new();
static SHARED_DESCRIPTORS: Once<SharedDescriptors> = Once::new();

static mut BSP_DF_STACK: [u8; DOUBLE_FAULT_STACK_SIZE] = [0; DOUBLE_FAULT_STACK_SIZE];

// ── Per-CPU storage (indexed by CpuId) ──────────────────────────────────────

struct ApStorage<T>(UnsafeCell<[MaybeUninit<T>; MAX_CPUS]>);

// SAFETY: Each CPU exclusively accesses its own slot; no other CPU reads or writes the same
// slot.
unsafe impl<T> Sync for ApStorage<T> {}

static AP_GDT: ApStorage<GlobalDescriptorTable> =
    ApStorage(UnsafeCell::new([const { MaybeUninit::uninit() }; MAX_CPUS]));
static AP_TSS: ApStorage<TaskStateSegment> =
    ApStorage(UnsafeCell::new([const { MaybeUninit::uninit() }; MAX_CPUS]));
static AP_DF_STACK: ApStorage<[u8; DOUBLE_FAULT_STACK_SIZE]> =
    ApStorage(UnsafeCell::new([const { MaybeUninit::uninit() }; MAX_CPUS]));

#[repr(C, align(16))]
struct ApStack([u8; AP_KERNEL_STACK_SIZE]);

static AP_KERNEL_STACK: ApStorage<ApStack> =
    ApStorage(UnsafeCell::new([const { MaybeUninit::uninit() }; MAX_CPUS]));

/// Per-CPU state shared between the naked syscall `entry` and Rust-side kernel-stack bookkeeping.
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
pub(super) struct PerCpuSyscall {
    /// Kernel stack the syscall `entry` switches onto; this is the current thread's kernel stack on
    /// this CPU, kept in sync with the TSS `RSP0` used by the interrupt-from-user path.
    pub(super) kernel_stack_top: u64,
    /// Saved user `RSP` handed from the syscall prologue to its `EntryFrame` construction.
    pub(super) user_stack_pointer: u64,
    /// Address of this CPU's loaded TSS, so Rust can update its `RSP0` on any CPU.
    tss_pointer: u64,
}

pub(super) const PER_CPU_KERNEL_STACK_TOP: usize = offset_of!(PerCpuSyscall, kernel_stack_top);
pub(super) const PER_CPU_USER_STACK_POINTER: usize = offset_of!(PerCpuSyscall, user_stack_pointer);

static PER_CPU_SYSCALL: ApStorage<PerCpuSyscall> =
    ApStorage(UnsafeCell::new([const { MaybeUninit::uninit() }; MAX_CPUS]));

/// Writes and publishes the per-CPU syscall slot and sets this CPU's `GS` base to it.
///
/// Called exactly once per active CPU during bring-up, before interrupts are enabled on that CPU.
fn initialize_per_cpu_syscall(index: usize, tss_pointer: u64) {
    let slot = unsafe { &mut (*PER_CPU_SYSCALL.0.get())[index] };
    // SAFETY: each CPU's slot is uninitialised and written exactly once during its own bring-up.
    slot.write(PerCpuSyscall {
        kernel_stack_top: 0,
        user_stack_pointer: 0,
        tss_pointer,
    });

    let slot = unsafe { &mut (*PER_CPU_SYSCALL.0.get())[index] };
    let base = slot.as_mut_ptr().cast::<u8>() as u64;
    GsBase::write(VirtAddr::new(base));
}

/// Returns the per-CPU syscall slot belonging to `index`. The caller must run on that CPU or hold
/// exclusive access to its brings-up/teardown.
pub(super) fn per_cpu_syscall_slot(index: usize) -> &'static mut PerCpuSyscall {
    let slot = unsafe { &mut (*PER_CPU_SYSCALL.0.get())[index] };
    // SAFETY: each CPU's slot is initialized in bring-up before it is ever used.
    unsafe { slot.assume_init_mut() }
}

/// Returns the per-CPU syscall slot belonging to the current CPU.
pub(super) fn current_cpu_syscall() -> &'static mut PerCpuSyscall {
    per_cpu_syscall_slot(cpu_map::current_id().get() as usize)
}

/// Returns the TSS loaded on the current CPU, for updating its `RSP0` on any CPU.
pub(super) fn current_cpu_tss() -> *mut TaskStateSegment {
    current_cpu_syscall().tss_pointer as *mut TaskStateSegment
}

struct Selectors {
    code: SegmentSelector,
    data: SegmentSelector,
    user_code: SegmentSelector,
    user_data: SegmentSelector,
    tss: SegmentSelector,
}

struct SharedDescriptors {
    code: Descriptor,
    data: Descriptor,
    user_data: Descriptor,
    user_code: Descriptor,
}

struct MutableTss(UnsafeCell<TaskStateSegment>);

// SAFETY: BSP runs alone during init; all mutation requires interrupts disabled.
unsafe impl Sync for MutableTss {}

pub(super) fn initialize(exception_handler: ExceptionHandler) {
    x86_64::instructions::interrupts::disable();
    assert!(!IDT.is_completed(), "architecture initialized twice");
    float::initialize();
    exception::register(exception_handler);

    let tss = BSP_TSS.call_once(|| MutableTss(UnsafeCell::new(create_tss())));
    let (gdt, selectors) = BSP_GDT.call_once(|| {
        // SAFETY: initialization is single-threaded and the TSS has a permanent address.
        create_gdt(unsafe { &*tss.0.get() })
    });
    gdt.load();

    // SAFETY: Both selectors reference descriptors in the loaded static GDT.
    unsafe {
        CS::set_reg(selectors.code);
        SS::set_reg(selectors.data);
        DS::set_reg(selectors.data);
        ES::set_reg(selectors.data);
        load_tss(selectors.tss);
    }

    IDT.call_once(create_idt).load();

    // Publish the BSP's per-CPU syscall slot and point GS at it.
    initialize_per_cpu_syscall(0, tss.0.get() as u64);

    // Store shared descriptors so APs can reuse them.
    SHARED_DESCRIPTORS.call_once(|| SharedDescriptors {
        code: Descriptor::kernel_code_segment(),
        data: Descriptor::kernel_data_segment(),
        user_data: Descriptor::user_data_segment(),
        user_code: Descriptor::user_code_segment(),
    });
}

/// Returns the shared IDT for APs to load.
pub(super) fn shared_idt() -> &'static InterruptDescriptorTable {
    IDT.get().expect("architecture not initialized")
}

#[allow(clippy::ptr_cast_constness)]
pub(super) fn register_ap() {
    // Register CPU identity first so `current_cpu_id` resolves on this CPU.
    cpu_map::register(cpu_map::read_current_apic_id());
}

/// Returns the top of this CPU's dedicated kernel stack.
///
/// The stack lives in the kernel image's `.bss`, so it is mapped under both the bootloader and
/// kernel page tables, allowing the AP to switch onto it before switching CR3.
pub(super) fn kernel_stack_top(cpu_id: CpuId) -> u64 {
    let index = cpu_id.get() as usize;
    let slot = unsafe { &mut (*AP_KERNEL_STACK.0.get())[index] };
    let ptr = slot.as_mut_ptr().cast::<u8>();
    // SAFETY: the slot is an initialized zeroed `.bss` array for the lifetime of the kernel.
    VirtAddr::from_ptr(ptr).as_u64() + AP_KERNEL_STACK_SIZE as u64
}

#[allow(clippy::ptr_cast_constness)]
pub(super) fn initialize_ap(kernel_stack_top: u64) {
    x86_64::instructions::interrupts::disable();
    float::initialize();
    let cpu_id = cpu_map::current_id();
    let index = cpu_id.get() as usize;

    let df_stack = unsafe { &mut (*AP_DF_STACK.0.get())[index] };
    // SAFETY: This CPU exclusively owns its slot; no other CPU reads or writes it.
    let df_stack_ptr = df_stack.as_mut_ptr();
    // SAFETY: Slot is uninitialised; we write it once now.
    unsafe { df_stack_ptr.write_bytes(0u8, 1) };

    let tss_slot = unsafe { &mut (*AP_TSS.0.get())[index] };
    // SAFETY: This CPU exclusively owns its slot.
    let tss_ptr = tss_slot.as_mut_ptr();
    // SAFETY: Slot is uninitialised; we write it once now.
    unsafe {
        tss_ptr.write(TaskStateSegment::new().tap_mut(|tss| {
            tss.privilege_stack_table[0] = VirtAddr::new(kernel_stack_top);
            tss.interrupt_stack_table[usize::from(DOUBLE_FAULT_IST_INDEX)] =
                VirtAddr::from_ptr(df_stack_ptr) + DOUBLE_FAULT_STACK_OFFSET;
        }));
    }
    // SAFETY: Just written above.
    let tss = unsafe { &*tss_ptr };

    let shared = SHARED_DESCRIPTORS
        .get()
        .expect("shared descriptors not initialized");
    let gdt_slot = unsafe { &mut (*AP_GDT.0.get())[index] };
    // SAFETY: This CPU exclusively owns its slot.
    let gdt_ptr = gdt_slot.as_mut_ptr();
    // SAFETY: Slot is uninitialised; we write it once now.
    unsafe { gdt_ptr.write(GlobalDescriptorTable::new()) };
    // SAFETY: Just written above.
    let gdt = unsafe { &mut *gdt_ptr };
    let code = gdt.append(shared.code);
    let data = gdt.append(shared.data);
    let _user_data = gdt.append(shared.user_data);
    let _user_code = gdt.append(shared.user_code);
    let tss_sel = gdt.append(Descriptor::tss_segment(tss));
    gdt.load();

    // SAFETY: Descriptors are resident in the freshly loaded GDT.
    unsafe {
        CS::set_reg(code);
        SS::set_reg(data);
        DS::set_reg(data);
        ES::set_reg(data);
        load_tss(tss_sel);
    }

    shared_idt().load();

    // Publish this AP's per-CPU syscall slot, point GS at it, and program this CPU's syscall MSRs.
    initialize_per_cpu_syscall(index, tss_ptr as u64);
    super::syscall::configure_cpu();

    // Establish this CPU's current-thread kernel stack in both the per-CPU slot and its own TSS.
    super::syscall::set_kernel_stack_top(kernel_stack_top);
    super::user::set_kernel_stack_top(kernel_stack_top);
}

/// Switches onto `stack_top`, loads `page_table_root_phys` into CR3, then jumps to
/// `continuation`, never returning to the caller.
///
/// The AP reaches this while still on the bootloader stack and page tables; afterwards it runs on
/// `stack_top` in the new address space. `continuation` is the first kernel code that runs there,
/// so everything after the switch is ordinary Rust.
///
/// # Safety
///
/// `stack_top` must be the top of a valid, mapped stack; `continuation` must never return; and
/// both the current and target page tables must map the stack and this code, since interrupts are
/// disabled to keep the stack switch and CR3 load atomic w.r.t. exceptions.
#[allow(clippy::ptr_cast_constness)]
pub(super) unsafe fn switch_stack_pt_and_call(
    stack_top: u64,
    page_table_root_phys: u64,
    continuation: extern "C" fn() -> !,
) -> ! {
    let continuation = continuation as usize;
    // `continuation` is a normal `extern "C" fn` compiled for the SysV ABI: when reached via
    // `call` it sees RSP = entry_rsp - 8 (the pushed return-address slot). The `jmp` below skips
    // that push, so switch RSP to `stack_top - 8` to reproduce the post-call entry alignment
    // (`stack_top` is 16-byte aligned, so the continuation sees RSP ≡ 8 (mod 16)) and reserve the
    // return-address slot, which the callee prologue and any aligned stack accesses rely on.
    let stack = stack_top - 8;

    // SAFETY: the caller satisfies the contract documented above.
    unsafe {
        core::arch::asm!(
            "mov rsp, {stack}",
            "mov cr3, {cr3}",
            "jmp {cont}",
            stack = in(reg) stack,
            cr3 = in(reg) page_table_root_phys,
            cont = in(reg) continuation,
            options(noreturn),
        );
    }
}

pub(super) fn user_selectors() -> (u64, u64) {
    let selectors = &BSP_GDT.get().expect("architecture not initialized").1;
    (
        u64::from(selectors.user_code.0),
        u64::from(selectors.user_data.0),
    )
}

pub(super) fn syscall_selectors() -> (
    SegmentSelector,
    SegmentSelector,
    SegmentSelector,
    SegmentSelector,
) {
    let selectors = &BSP_GDT.get().expect("architecture not initialized").1;
    (
        selectors.user_code,
        selectors.user_data,
        selectors.code,
        selectors.data,
    )
}

fn create_tss() -> TaskStateSegment {
    let stack_start = VirtAddr::from_ptr(core::ptr::addr_of!(BSP_DF_STACK));
    TaskStateSegment::new().tap_mut(|tss| {
        tss.interrupt_stack_table[usize::from(DOUBLE_FAULT_IST_INDEX)] =
            stack_start + DOUBLE_FAULT_STACK_OFFSET;
    })
}

fn create_gdt(tss: &'static TaskStateSegment) -> (GlobalDescriptorTable, Selectors) {
    let mut gdt = GlobalDescriptorTable::new();
    let code = gdt.append(Descriptor::kernel_code_segment());
    let data = gdt.append(Descriptor::kernel_data_segment());
    let user_data = gdt.append(Descriptor::user_data_segment());
    let user_code = gdt.append(Descriptor::user_code_segment());
    let tss = gdt.append(Descriptor::tss_segment(tss));
    (
        gdt,
        Selectors {
            code,
            data,
            user_code,
            user_data,
            tss,
        },
    )
}

fn create_idt() -> InterruptDescriptorTable {
    InterruptDescriptorTable::new().tap_mut(|idt| {
        idt.divide_error.set_handler_fn(exception::divide_error);
        idt.invalid_opcode.set_handler_fn(exception::invalid_opcode);
        idt.general_protection_fault
            .set_handler_fn(exception::general_protection_fault);
        idt.page_fault.set_handler_fn(exception::page_fault);
        idt[interrupt::TIMER_VECTOR].set_handler_fn(interrupt::timer);
        idt[interrupt::ERROR_VECTOR].set_handler_fn(interrupt::error);
        idt[interrupt::SPURIOUS_VECTOR].set_handler_fn(interrupt::spurious);
        idt[interrupt::RESCHEDULE_VECTOR].set_handler_fn(interrupt::reschedule);
        idt[interrupt::IRQ_VECTOR_BASE].set_handler_fn(interrupt::irq0);
        idt[interrupt::IRQ_VECTOR_BASE + 1].set_handler_fn(interrupt::irq1);
        idt[interrupt::IRQ_VECTOR_BASE + 2].set_handler_fn(interrupt::irq2);
        idt[interrupt::IRQ_VECTOR_BASE + 3].set_handler_fn(interrupt::irq3);
        idt[interrupt::IRQ_VECTOR_BASE + 4].set_handler_fn(interrupt::irq4);
        idt[interrupt::IRQ_VECTOR_BASE + 5].set_handler_fn(interrupt::irq5);
        idt[interrupt::IRQ_VECTOR_BASE + 6].set_handler_fn(interrupt::irq6);
        idt[interrupt::IRQ_VECTOR_BASE + 7].set_handler_fn(interrupt::irq7);
        idt[interrupt::IRQ_VECTOR_BASE + 8].set_handler_fn(interrupt::irq8);
        idt[interrupt::IRQ_VECTOR_BASE + 9].set_handler_fn(interrupt::irq9);
        idt[interrupt::IRQ_VECTOR_BASE + 10].set_handler_fn(interrupt::irq10);
        idt[interrupt::IRQ_VECTOR_BASE + 11].set_handler_fn(interrupt::irq11);
        idt[interrupt::IRQ_VECTOR_BASE + 12].set_handler_fn(interrupt::irq12);
        idt[interrupt::IRQ_VECTOR_BASE + 13].set_handler_fn(interrupt::irq13);
        idt[interrupt::IRQ_VECTOR_BASE + 14].set_handler_fn(interrupt::irq14);
        idt[interrupt::IRQ_VECTOR_BASE + 15].set_handler_fn(interrupt::irq15);

        // SAFETY: The configured IST entry points at the static double-fault stack.
        unsafe {
            idt.double_fault
                .set_handler_fn(exception::double_fault)
                .set_stack_index(DOUBLE_FAULT_IST_INDEX);
        }
    })
}
