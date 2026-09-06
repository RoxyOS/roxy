use clap::ValueEnum;

/// A kernel architecture the task runner can build and exercise.
///
/// The enum centralizes every architecture-derived value (Rust target triple, artifact name
/// suffix, QEMU binary and machine) so the rest of the task runner never hardcodes an arch.
/// Only [`Arch::X86_64`] is currently end-to-end runnable; [`Arch::Aarch64`] carries the
/// well-defined naming and build plumbing but its boot (firmware/EFI) and userspace toolchain
/// are not yet wired.
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum Arch {
    #[value(name = "x86_64")]
    X86_64,
    Aarch64,
}

impl Arch {
    /// The Rust target triple used to build the kernel.
    pub(crate) fn triple(self) -> &'static str {
        match self {
            Arch::X86_64 => "x86_64-unknown-none",
            Arch::Aarch64 => "aarch64-unknown-none",
        }
    }

    /// Short architecture name used in artifact file names.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Arch::X86_64 => "x86_64",
            Arch::Aarch64 => "aarch64",
        }
    }

    /// QEMU system emulator binary for this architecture.
    pub(crate) fn qemu_runner(self) -> &'static str {
        match self {
            Arch::X86_64 => "qemu-system-x86_64",
            Arch::Aarch64 => "qemu-system-aarch64",
        }
    }

    /// QEMU machine model used when launching this architecture.
    pub(crate) fn qemu_machine(self) -> &'static str {
        match self {
            Arch::X86_64 => "q35",
            Arch::Aarch64 => "virt",
        }
    }
}
