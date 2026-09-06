use clap::{Parser, Subcommand};

use crate::arch::Arch;

#[derive(Debug, Parser)]
#[command(name = "xtask", about = "Roxy OS development tasks")]
pub(crate) struct Cli {
    /// Target kernel architecture.
    #[arg(long, value_enum, default_value = "x86_64")]
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
}
