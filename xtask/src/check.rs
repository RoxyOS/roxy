use anyhow::Result;

pub(crate) fn run() -> Result<()> {
    println!("==> Checking formatting");
    crate::cmd!("cargo fmt --all --check")?;

    println!("==> Checking workspace");
    crate::cmd!("cargo check --workspace --all-targets")?;

    println!("==> Linting workspace");
    crate::cmd!("cargo clippy --workspace --all-targets -- -D warnings")?;

    println!("==> Checking release kernel");
    crate::cmd!(
        "cargo check --package roxy-kernel --features kernel --target x86_64-unknown-none --release"
    )?;
    crate::cmd!(
        "cargo clippy --package roxy-kernel --features kernel --target x86_64-unknown-none --release -- -D warnings"
    )?;

    println!("==> Checking release test kernel");
    crate::cmd!(
        "cargo check --package roxy-kernel --features kernel,kernel-test --target x86_64-unknown-none --release"
    )?;
    crate::cmd!(
        "cargo clippy --package roxy-kernel --features kernel,kernel-test --target x86_64-unknown-none --release -- -D warnings"
    )?;

    println!("==> Checking diff whitespace");
    crate::cmd!("git diff --check")?;

    println!("==> All checks passed");

    Ok(())
}
