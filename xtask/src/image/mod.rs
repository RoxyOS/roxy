mod iso;
mod limine;
mod qemu;

use std::path::{Path, PathBuf};

use anyhow::Result;

pub(crate) fn build_iso(kernel: &Path) -> Result<()> {
    println!("==> Building boot image");

    create_iso(kernel)?;
    Ok(())
}

pub(crate) fn run(kernel: &Path) -> Result<()> {
    let image = create_iso(kernel)?;

    qemu::run(&image)
}

fn create_iso(kernel: &Path) -> Result<PathBuf> {
    let root = output_root();
    let limine = limine::prepare(&root)?;
    iso::build(&root, kernel, &limine)
}

fn output_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("target/roxy")
}
