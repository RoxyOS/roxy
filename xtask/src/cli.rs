use clap::{Parser, Subcommand, ValueEnum};

use crate::arch::Arch;

/// Kernel build profile used by debugging launches. `Dev` compiles unoptimized with DWARF for
/// source-level GDB; `Release` is the optimized build the kernel normally ships.
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum Profile {
    Dev,
    Release,
}

impl Profile {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Dev => "dev",
            Self::Release => "release",
        }
    }
}

#[derive(Debug, Parser)]
#[command(name = "xtask", about = "Roxy OS development tasks")]
pub(crate) struct Cli {
    /// Target kernel architecture.
    #[arg(long, value_enum, global = true, default_value = "x86_64")]
    pub(crate) arch: Arch,

    #[command(subcommand)]
    pub(crate) arg: Option<Arg>,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Arg {
    Check,
    Image,
    Rootfs,
    Run,
    Test,
    Debug {
        /// Kernel build profile: `dev` produces a DWARF-enabled kernel for source-level GDB.
        #[arg(long, value_enum)]
        profile: Profile,
    },
}
