mod abi;
mod action;
mod build_kernel;
mod cli;
mod image;
mod utils;

pub(crate) use build_kernel::{build_kernel, build_test_kernel};

use anyhow::Result;
use clap::Parser;
use cli::{AbiArg, Arg, Cli};

fn main() -> Result<()> {
    let cli = Cli::parse();

    if let Some(arg) = cli.arg {
        match arg {
            Arg::Abi { arg: AbiArg::Build } => abi::build(),
            Arg::Abi { arg: AbiArg::Check } => abi::check(),
            Arg::Abi {
                arg: AbiArg::Generate,
            } => abi::generate(),
            Arg::Image => action::image(),
            Arg::Run => action::run(),
            Arg::Test => action::test(),
        }?;
    }
    Ok(())
}
