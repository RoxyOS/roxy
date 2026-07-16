use spin::Once;
use tap::Tap;
use x86_64::{
    VirtAddr,
    instructions::{
        segmentation::{CS, Segment},
        tables::load_tss,
    },
    structures::{
        gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector},
        idt::InterruptDescriptorTable,
        tss::TaskStateSegment,
    },
};

use crate::ExceptionHandler;

use super::exception;

const DOUBLE_FAULT_IST_INDEX: u16 = 0;
const DOUBLE_FAULT_STACK_SIZE: usize = 4096 * 5;
const DOUBLE_FAULT_STACK_OFFSET: u64 = 4096 * 5;

static TSS: Once<TaskStateSegment> = Once::new();
static GDT: Once<(GlobalDescriptorTable, Selectors)> = Once::new();
static IDT: Once<InterruptDescriptorTable> = Once::new();

static mut DOUBLE_FAULT_STACK: [u8; DOUBLE_FAULT_STACK_SIZE] = [0; DOUBLE_FAULT_STACK_SIZE];

struct Selectors {
    code: SegmentSelector,
    tss: SegmentSelector,
}

pub(super) fn initialize(exception_handler: ExceptionHandler) {
    assert!(!IDT.is_completed(), "architecture initialized twice");
    exception::register(exception_handler);

    let tss = TSS.call_once(create_tss);
    let (gdt, selectors) = GDT.call_once(|| create_gdt(tss));
    gdt.load();

    // SAFETY: Both selectors reference descriptors in the loaded static GDT.
    unsafe {
        CS::set_reg(selectors.code);
        load_tss(selectors.tss);
    }

    IDT.call_once(create_idt).load();
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
    let tss = gdt.append(Descriptor::tss_segment(tss));
    (gdt, Selectors { code, tss })
}

fn create_idt() -> InterruptDescriptorTable {
    InterruptDescriptorTable::new().tap_mut(|idt| {
        idt.divide_error.set_handler_fn(exception::divide_error);
        idt.invalid_opcode.set_handler_fn(exception::invalid_opcode);
        idt.general_protection_fault
            .set_handler_fn(exception::general_protection_fault);
        idt.page_fault.set_handler_fn(exception::page_fault);

        // SAFETY: The configured IST entry points at the static double-fault stack.
        unsafe {
            idt.double_fault
                .set_handler_fn(exception::double_fault)
                .set_stack_index(DOUBLE_FAULT_IST_INDEX);
        }
    })
}
