use anyhow::Result;

use crate::{build_kernel, image};

pub(crate) fn image() -> Result<()> {
    let kernel = build_kernel()?;
    image::build_iso(&kernel)
}

pub(crate) fn run() -> Result<()> {
    let kernel = build_kernel()?;
    image::run(&kernel)
}
