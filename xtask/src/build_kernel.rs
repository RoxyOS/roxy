use std::path::PathBuf;

use anyhow::{Result, ensure};

use crate::arch::Arch;

pub(crate) fn build_kernel(arch: Arch) -> Result<PathBuf> {
    println!("==> Building kernel ({})", arch.triple());
    let triple = arch.triple();
    crate::cmd!("cargo build --package kernel-main --features kernel --target {triple} --release")?;

    kernel_path(arch)
}

pub(crate) fn build_test_kernel(arch: Arch) -> Result<PathBuf> {
    println!("==> Building test kernel ({})", arch.triple());
    let triple = arch.triple();
    crate::cmd!(
        "cargo build --package kernel-main --features kernel,kernel-test --target {triple} --release"
    )?;

    kernel_path(arch)
}

fn kernel_path(arch: Arch) -> Result<PathBuf> {
    let kernel = workspace_root().join(format!("target/{}/release/kernel-main", arch.triple()));
    ensure!(
        kernel.is_file(),
        "kernel ELF was not produced at {}",
        kernel.display()
    );

    Ok(kernel)
}

pub(crate) fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask must be inside the workspace")
        .to_owned()
}
