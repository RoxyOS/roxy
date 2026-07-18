mod action;
mod build_kernel;
mod check;
mod cli;
mod image;
mod utils;

pub(crate) use build_kernel::{build_kernel, build_test_kernel};

use anyhow::Result;
use clap::Parser;
use cli::{Arg, Cli};

fn main() -> Result<()> {
    let cli = Cli::parse();

    if let Some(arg) = cli.arg {
        match arg {
            Arg::Check => check::run(),
            Arg::Image => action::image(),
            Arg::Run => action::run(),
            Arg::Test => action::test(),
        }?;
    }
    Ok(())
}
