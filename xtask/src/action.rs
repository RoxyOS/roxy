use anyhow::Result;

use crate::{build_kernel, build_test_kernel, image, rootfs};

pub(crate) fn rootfs() -> Result<()> {
    rootfs::build()?;

    Ok(())
}

pub(crate) fn image() -> Result<()> {
    let rootfs = rootfs::get_or_build()?;
    let kernel = build_kernel()?;

    image::build_iso(&kernel, &rootfs)
}

pub(crate) fn run() -> Result<()> {
    let rootfs = rootfs::get_or_build()?;
    let kernel = build_kernel()?;

    image::run(&kernel, &rootfs)
}

pub(crate) fn test() -> Result<()> {
    let rootfs = rootfs::get_or_build()?;
    let kernel = build_test_kernel()?;

    image::test(&kernel, &rootfs)
}
