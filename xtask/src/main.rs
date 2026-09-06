mod action;
mod arch;
mod build_kernel;
mod check;
mod cli;
mod image;
mod rootfs;
mod utils;

pub(crate) use build_kernel::{build_kernel, build_test_kernel};

use anyhow::Result;
use clap::Parser;
use cli::{Arg, Cli};

fn main() -> Result<()> {
    let cli = Cli::parse();

    if let Some(arg) = cli.arg {
        match arg {
            Arg::Check => check::run(cli.arch),
            Arg::Image => action::image(cli.arch),
            Arg::Rootfs => action::rootfs(cli.arch),
            Arg::Run => action::run(cli.arch),
            Arg::Test => action::test(cli.arch),
        }?;
    }

    Ok(())
}
