use anyhow::Result;

use crate::{build_kernel, build_test_kernel, image};

pub(crate) fn image() -> Result<()> {
    let kernel = build_kernel()?;
    image::build_iso(&kernel)
}

pub(crate) fn run() -> Result<()> {
    let kernel = build_kernel()?;
    image::run(&kernel)
}

pub(crate) fn test() -> Result<()> {
    let kernel = build_test_kernel()?;
    image::test(&kernel)
}
