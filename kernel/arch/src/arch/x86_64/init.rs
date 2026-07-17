use core::{arch::naked_asm, cell::UnsafeCell};

use spin::Once;
use tap::Tap;
use x86_64::{
    VirtAddr,
    instructions::{
        segmentation::{CS, DS, ES, SS, Segment},
        tables::load_tss,
    },
    registers::{
        model_specific::{Efer, EferFlags, LStar, SFMask, Star},
        rflags::RFlags,
    },
    structures::{
        gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector},
        idt::InterruptDescriptorTable,
        tss::TaskStateSegment,
    },
};

use crate::{ExceptionHandler, LocalInterruptHandler};

use super::{exception, interrupt};

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

pub(super) fn initialize(
    exception_handler: ExceptionHandler,
    local_interrupt_handler: LocalInterruptHandler,
) {
    x86_64::instructions::interrupts::disable();
    assert!(!IDT.is_completed(), "architecture initialized twice");
    exception::register(exception_handler);
    interrupt::register(local_interrupt_handler);

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

pub(super) unsafe fn enter_user(
    user_instruction_pointer: u64,
    user_stack_pointer: u64,
    kernel_stack_top: u64,
) -> ! {
    assert!(!x86_64::instructions::interrupts::are_enabled());
    let tss = TSS.get().expect("architecture not initialized");
    // SAFETY: interrupts are disabled on the single supported CPU, so no concurrent TSS access exists.
    unsafe { (*tss.0.get()).privilege_stack_table[0] = VirtAddr::new(kernel_stack_top) };

    let selectors = &GDT.get().expect("architecture not initialized").1;
    // SAFETY: initialization installed these ring-3 descriptors and the caller validates mappings.
    unsafe {
        iret_to_user(
            user_instruction_pointer,
            user_stack_pointer,
            u64::from(selectors.user_code.0),
            u64::from(selectors.user_data.0),
        )
    }
}

pub(super) unsafe fn configure_syscall(entry: u64) {
    let selectors = &GDT.get().expect("architecture not initialized").1;
    // SAFETY: architecture initialization established long mode and the configured entry is valid.
    unsafe { Efer::update(|flags| flags.insert(EferFlags::SYSTEM_CALL_EXTENSIONS)) };

    Star::write(
        selectors.user_code,
        selectors.user_data,
        selectors.code,
        selectors.data,
    )
    .expect("invalid syscall segment layout");
    LStar::write(VirtAddr::new(entry));
    SFMask::write(RFlags::INTERRUPT_FLAG);
}

#[unsafe(naked)]
unsafe extern "C" fn iret_to_user(
    _user_instruction_pointer: u64,
    _user_stack_pointer: u64,
    _code_selector: u64,
    _data_selector: u64,
) -> ! {
    naked_asm!(
        "mov ax, cx",
        "mov ds, ax",
        "mov es, ax",
        "push rcx",
        "push rsi",
        "push 0x202",
        "push rdx",
        "push rdi",
        "iretq",
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
