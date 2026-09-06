mod iso;
mod limine;
mod qemu;

use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::arch::Arch;

#[derive(Clone, Copy)]
pub(super) enum Mode {
    Production,
    Test,
}

pub(crate) fn build_iso(kernel: &Path, rootfs: &Path, arch: Arch) -> Result<()> {
    println!("==> Building boot image");

    create_iso(kernel, rootfs, Mode::Production, arch)?;

    Ok(())
}

pub(crate) fn run(kernel: &Path, rootfs: &Path, arch: Arch) -> Result<()> {
    let image = create_iso(kernel, rootfs, Mode::Production, arch)?;

    qemu::run(&image, arch)
}

pub(crate) fn test(kernel: &Path, rootfs: &Path, arch: Arch) -> Result<()> {
    let image = create_iso(kernel, rootfs, Mode::Test, arch)?;

    qemu::test(&image, arch)
}

fn create_iso(kernel: &Path, rootfs: &Path, mode: Mode, arch: Arch) -> Result<PathBuf> {
    let root = output_root();
    let limine = limine::prepare(&root)?;
    iso::build(&root, kernel, rootfs, &limine, mode, arch)
}

fn output_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("target/roxy")
}
