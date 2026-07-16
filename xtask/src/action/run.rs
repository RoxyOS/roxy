use anyhow::Result;

use crate::{build_kernel, image_runner};

pub(crate) fn run() -> Result<()> {
    let kernel = build_kernel()?;

    println!("==> Starting virtual machine");
    image_runner(&kernel)?.run()?;
    Ok(())
}
