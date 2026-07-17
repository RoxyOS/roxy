use std::path::PathBuf;

use anyhow::{Result, ensure};

pub(crate) fn build_kernel() -> Result<PathBuf> {
    println!("==> Building kernel");
    crate::cmd!(
        "cargo build --package roxy-kernel --features kernel --target x86_64-unknown-none --release"
    )?;

    kernel_path()
}

pub(crate) fn build_test_kernel() -> Result<PathBuf> {
    println!("==> Building test kernel");
    crate::cmd!(
        "cargo build --package roxy-kernel --features kernel,kernel-test --target x86_64-unknown-none --release"
    )?;

    kernel_path()
}

fn kernel_path() -> Result<PathBuf> {
    let kernel = workspace_root().join("target/x86_64-unknown-none/release/roxy-kernel");
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
