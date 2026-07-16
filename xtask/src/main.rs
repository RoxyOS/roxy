mod action;
mod build_kernel;
mod cli;
mod image_runner;
mod utils;

pub(crate) use build_kernel::build_kernel;
pub(crate) use image_runner::image_runner;

use anyhow::Result;
use clap::Parser;
use cli::{Arg, Cli};

fn main() -> Result<()> {
    let cli = Cli::parse();

    if let Some(arg) = cli.arg {
        match arg {
            Arg::Image => action::image(),
            Arg::Run => action::run(),
        }?;
    }
    Ok(())
}
