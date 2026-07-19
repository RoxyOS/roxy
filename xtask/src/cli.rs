use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "xtask", about = "Roxy OS development tasks")]
pub(crate) struct Cli {
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
