use core::cell::UnsafeCell;

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

use crate::ExceptionHandler;

use super::{exception, float, interrupt};

const DOUBLE_FAULT_IST_INDEX: u16 = 0;
const DOUBLE_FAULT_STACK_SIZE: usize = 4096 * 5;
const DOUBLE_FAULT_STACK_OFFSET: u64 = 4096 * 5;

static TSS: Once<MutableTss> = Once::new();
static GDT: Once<(GlobalDescriptorTable, Selectors)> = Once::new();
static IDT: Once<InterruptDescriptorTable> = Once::new();

static mut DOUBLE_FAULT_STACK: [u8; DOUBLE_FAULT_STACK_SIZE] = [0; DOUBLE_FAULT_STACK_SIZE];

struct Selectors {
    code: SegmentSelector,
    data: SegmentSelector,
    user_code: SegmentSelector,
    user_data: SegmentSelector,
    tss: SegmentSelector,
}

struct MutableTss(UnsafeCell<TaskStateSegment>);

// SAFETY: Stage 8 runs only the BSP, and all mutation requires interrupts to be disabled.
unsafe impl Sync for MutableTss {}

pub(super) fn initialize(exception_handler: ExceptionHandler) {
    x86_64::instructions::interrupts::disable();
    assert!(!IDT.is_completed(), "architecture initialized twice");
    float::initialize();
    exception::register(exception_handler);

    let tss = TSS.call_once(|| MutableTss(UnsafeCell::new(create_tss())));
    let (gdt, selectors) = GDT.call_once(|| {
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
}

pub(super) fn tss_pointer() -> *mut TaskStateSegment {
    TSS.get().expect("architecture not initialized").0.get()
}

pub(super) fn user_selectors() -> (u64, u64) {
    let selectors = &GDT.get().expect("architecture not initialized").1;
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
    let selectors = &GDT.get().expect("architecture not initialized").1;
    (
        selectors.user_code,
        selectors.user_data,
        selectors.code,
        selectors.data,
    )
}

fn create_tss() -> TaskStateSegment {
    let stack_start = VirtAddr::from_ptr(core::ptr::addr_of!(DOUBLE_FAULT_STACK));
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

        // SAFETY: The configured IST entry points at the static double-fault stack.
        unsafe {
            idt.double_fault
                .set_handler_fn(exception::double_fault)
                .set_stack_index(DOUBLE_FAULT_IST_INDEX);
        }
    })
}
