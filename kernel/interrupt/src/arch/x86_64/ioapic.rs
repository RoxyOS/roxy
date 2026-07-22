use core::ptr::NonNull;

use acpi::{
    AcpiHandler, AcpiTables, PhysicalMapping,
    madt::{Madt, MadtEntry},
};
use x2apic::ioapic::{IoApic, IrqFlags, IrqMode, RedirectionTableEntry};
use x86_64::instructions::port::Port;

use roxy_arch::{Architecture, CurrentArchitectureBackend, Interrupt, IrqLine};
use roxy_cpu::CpuLocal;
use roxy_utils::Lock;

use crate::InterruptPlatformInfo;

const LAST_ISA_IRQ: u32 = 15;

static IO_APIC: CpuLocal<Lock<ConfiguredIoApic>> = CpuLocal::new();

struct ConfiguredIoApic {
    io_apic: IoApic,
    gsi_base: u32,
    max_entry: u8,
}

// SAFETY: The kernel currently initializes and accesses the IOAPIC only on the BSP.
unsafe impl Send for ConfiguredIoApic {}

#[derive(Clone, Copy)]
struct HhdmAcpiHandler {
    offset: u64,
}

impl AcpiHandler for HhdmAcpiHandler {
    unsafe fn map_physical_region<T>(
        &self,
        physical_address: usize,
        size: usize,
    ) -> PhysicalMapping<Self, T> {
        let virtual_address = self
            .offset
            .checked_add(physical_address as u64)
            .expect("ACPI physical address overflows HHDM");
        let pointer = NonNull::new(virtual_address as *mut T).expect("ACPI mapping is null");

        // SAFETY: Limine's HHDM permanently maps every physical region used by ACPI.
        unsafe { PhysicalMapping::new(physical_address, pointer, size, size, *self) }
    }

    fn unmap_physical_region<T>(_region: &PhysicalMapping<Self, T>) {}
}

pub(super) fn initialize(platform: InterruptPlatformInfo, destination: u32) {
    let handler = HhdmAcpiHandler {
        offset: platform.hhdm_offset,
    };

    let rsdp_address =
        usize::try_from(platform.rsdp_address).expect("RSDP address overflows usize");
    let tables =
        unsafe { AcpiTables::from_rsdp(handler, rsdp_address) }.expect("parse ACPI tables");
    let madt = tables.find_table::<Madt>().expect("ACPI MADT is missing");

    let mut io_apic = find_io_apic(madt.get(), platform.hhdm_offset);

    configure_redirection_table(&mut io_apic, destination);
    mask_legacy_pic();

    IO_APIC.initialize_current(Lock::new(io_apic));
}

fn find_io_apic(madt: core::pin::Pin<&Madt>, hhdm_offset: u64) -> ConfiguredIoApic {
    let mut selected = None;
    for entry in madt.entries() {
        match entry {
            MadtEntry::IoApic(entry) => {
                let address = entry.io_apic_address;
                let gsi_base = entry.global_system_interrupt_base;

                if gsi_base == 0 && selected.is_none() {
                    let mapped_address = hhdm_offset
                        .checked_add(u64::from(address))
                        .expect("IOAPIC address overflows HHDM");

                    let mut io_apic = unsafe { IoApic::new(mapped_address) };
                    let max_entry = unsafe { io_apic.max_table_entry() };

                    if u32::from(max_entry) >= LAST_ISA_IRQ {
                        selected = Some(ConfiguredIoApic {
                            io_apic,
                            gsi_base,
                            max_entry,
                        });
                    }
                }
            }
            MadtEntry::InterruptSourceOverride(entry) => {
                let isa_source = entry.irq;
                let gsi = entry.global_system_interrupt;
                let flags = entry.flags;

                assert!(
                    !(matches!(isa_source, 1 | 12) && (gsi != u32::from(isa_source) || flags != 0)),
                    "unsupported ACPI override for ISA IRQ {isa_source}"
                );
            }
            _ => {}
        }
    }

    selected.expect("no IOAPIC covers ISA IRQ0..IRQ15")
}

fn configure_redirection_table(io_apic: &mut ConfiguredIoApic, destination: u32) {
    let destination =
        u8::try_from(destination).expect("BSP APIC ID does not fit IOAPIC destination");

    for number in 0..=LAST_ISA_IRQ {
        let number = u8::try_from(number).expect("ISA IRQ number does not fit IOAPIC entry");
        let line = IrqLine::new(number).expect("ISA IRQ number is invalid");
        let mut entry = RedirectionTableEntry::default();

        entry.set_vector(CurrentArchitectureBackend::interrupt_vector(
            Interrupt::Irq(line),
        ));
        entry.set_mode(IrqMode::Fixed);
        entry.set_flags(IrqFlags::MASKED);
        entry.set_dest(destination);

        unsafe {
            io_apic.io_apic.set_table_entry(number, entry);
        }
    }
}

fn mask_legacy_pic() {
    // SAFETY: The legacy PIC command/data ports are fixed by the ISA specification.
    unsafe {
        Port::<u8>::new(0x21).write(0xff);
        Port::<u8>::new(0xa1).write(0xff);
    }
}

pub(super) fn mask(line: IrqLine) {
    with_io_apic(|io_apic| {
        let entry = table_index(io_apic, line);
        // SAFETY: table_index validates the IOAPIC redirection-table bounds.
        unsafe { io_apic.io_apic.disable_irq(entry) };
    });
}

pub(super) fn unmask(line: IrqLine) {
    with_io_apic(|io_apic| {
        let entry = table_index(io_apic, line);
        // SAFETY: table_index validates the IOAPIC redirection-table bounds.
        unsafe { io_apic.io_apic.enable_irq(entry) };
    });
}

fn table_index(io_apic: &ConfiguredIoApic, line: IrqLine) -> u8 {
    let gsi = io_apic
        .gsi_base
        .checked_add(u32::from(line.number()))
        .expect("IRQ GSI overflows");
    let index = gsi
        .checked_sub(io_apic.gsi_base)
        .expect("IRQ is below IOAPIC GSI base");
    assert!(
        index <= u32::from(io_apic.max_entry),
        "IRQ is outside IOAPIC range"
    );
    u8::try_from(index).expect("IOAPIC entry index does not fit u8")
}

fn with_io_apic<T>(function: impl FnOnce(&mut ConfiguredIoApic) -> T) -> T {
    assert!(!CurrentArchitectureBackend::interrupts_enabled());
    function(&mut IO_APIC.get().lock())
}
