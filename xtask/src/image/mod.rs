mod iso;
mod limine;
mod qemu;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Result, anyhow};

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

pub(crate) fn debug(
    kernel: &Path,
    rootfs: &Path,
    arch: Arch,
    profile: crate::cli::Profile,
) -> Result<()> {
    let image = create_iso(kernel, rootfs, Mode::Production, arch)?;
    let debug_dir = new_debug_dir()?;

    qemu::debug(&image, kernel, rootfs, arch, profile, &debug_dir)
}

fn new_debug_dir() -> Result<PathBuf> {
    let root = output_root().join("agent-debug");
    fs::create_dir_all(&root)?;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| anyhow!("system clock is before UNIX epoch: {error}"))?
        .as_nanos();
    let directory = root.join(format!("run-{timestamp}"));
    fs::create_dir(&directory)?;
    Ok(directory)
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
