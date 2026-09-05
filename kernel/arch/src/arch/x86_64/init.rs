use core::{cell::UnsafeCell, mem::MaybeUninit};
use spin::Once;
use tap::Tap;
use x86_64::{
    VirtAddr,
    instructions::{
        segmentation::{CS, DS, ES, SS, Segment},
        tables::load_tss,
    },
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
pub(super) fn initialize_ap(kernel_stack_top: u64) {
    x86_64::instructions::interrupts::disable();

    // Register CPU identity first so `current_cpu_id` works.
    cpu_map::register(cpu_map::read_current_apic_id());
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

    super::syscall::set_kernel_stack_top(kernel_stack_top);
    super::user::set_kernel_stack_top(kernel_stack_top);

    // TODO(smp-pagetable): The AP still runs under the bootloader's page tables (the BSP switched
    // to its own during memory init). Until this AP switches CR3 to the kernel page tables, it
    // cannot use kernel heap or device mappings.
}

pub(super) fn tss_pointer() -> *mut TaskStateSegment {
    BSP_TSS.get().expect("architecture not initialized").0.get()
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
