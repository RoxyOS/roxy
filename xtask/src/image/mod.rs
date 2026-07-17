mod iso;
mod limine;
mod qemu;

use std::path::{Path, PathBuf};

use anyhow::Result;

#[derive(Clone, Copy)]
pub(super) enum Mode {
    Production,
    Test,
}

pub(crate) fn build_iso(kernel: &Path) -> Result<()> {
    println!("==> Building boot image");

    create_iso(kernel, Mode::Production)?;
    Ok(())
}

pub(crate) fn run(kernel: &Path) -> Result<()> {
    let image = create_iso(kernel, Mode::Production)?;

    qemu::run(&image)
}

pub(crate) fn test(kernel: &Path) -> Result<()> {
    let image = create_iso(kernel, Mode::Test)?;

    qemu::test(&image)
}

fn create_iso(kernel: &Path, mode: Mode) -> Result<PathBuf> {
    let root = output_root();
    let limine = limine::prepare(&root)?;
    iso::build(&root, kernel, &limine, mode)
}

fn output_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("target/roxy")
}
