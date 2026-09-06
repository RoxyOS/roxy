use anyhow::Result;

use crate::arch::Arch;

pub(crate) fn run(arch: Arch) -> Result<()> {
    let triple = arch.triple();

    println!("==> Checking formatting");
    crate::cmd!("cargo fmt --all --check")?;

    println!("==> Checking workspace");
    crate::cmd!("cargo check --workspace --all-targets")?;

    println!("==> Linting workspace");
    crate::cmd!("cargo clippy --workspace --all-targets -- -D warnings")?;

    println!("==> Checking release kernel");
    crate::cmd!("cargo check --package kernel-main --features kernel --target {triple} --release")?;
    crate::cmd!(
        "cargo clippy --package kernel-main --features kernel --target {triple} --release -- -D warnings"
    )?;

    println!("==> Checking release test kernel");
    crate::cmd!(
        "cargo check --package kernel-main --features kernel,kernel-test --target {triple} --release"
    )?;
    crate::cmd!(
        "cargo clippy --package kernel-main --features kernel,kernel-test --target {triple} --release -- -D warnings"
    )?;

    println!("==> Checking diff whitespace");
    crate::cmd!("git diff --check")?;

    println!("==> All checks passed");

    Ok(())
}
