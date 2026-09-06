use anyhow::Result;

use crate::{arch::Arch, build_kernel, build_test_kernel, image, rootfs};

pub(crate) fn rootfs(arch: Arch) -> Result<()> {
    rootfs::build(arch)?;

    Ok(())
}

pub(crate) fn image(arch: Arch) -> Result<()> {
    let rootfs = rootfs::get_or_build(arch)?;
    let kernel = build_kernel(arch)?;

    image::build_iso(&kernel, &rootfs, arch)
}

pub(crate) fn run(arch: Arch) -> Result<()> {
    let rootfs = rootfs::get_or_build(arch)?;
    let kernel = build_kernel(arch)?;

    image::run(&kernel, &rootfs, arch)
}

pub(crate) fn test(arch: Arch) -> Result<()> {
    let rootfs = rootfs::get_or_build(arch)?;
    let kernel = build_test_kernel(arch)?;

    image::test(&kernel, &rootfs, arch)
}
