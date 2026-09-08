use std::path::PathBuf;

use anyhow::{Result, ensure};

use crate::arch::Arch;
use crate::cli::Profile;

pub(crate) fn build_kernel(arch: Arch, profile: Profile) -> Result<PathBuf> {
    println!("==> Building kernel ({})", arch.triple());
    let triple = arch.triple();
    match profile {
        Profile::Release => {
            crate::cmd!(
                "cargo build --package kernel-main --features kernel --target {triple} --release"
            )?;
        }
        Profile::Dev => {
            crate::cmd!("cargo build --package kernel-main --features kernel --target {triple}")?;
        }
    }

    kernel_path(arch, profile)
}

pub(crate) fn build_test_kernel(arch: Arch) -> Result<PathBuf> {
    println!("==> Building test kernel ({})", arch.triple());
    let triple = arch.triple();
    crate::cmd!(
        "cargo build --package kernel-main --features kernel,kernel-test --target {triple} --release"
    )?;

    kernel_path(arch, Profile::Release)
}

fn kernel_path(arch: Arch, profile: Profile) -> Result<PathBuf> {
    let profile_dir = match profile {
        Profile::Release => "release",
        Profile::Dev => "debug",
    };
    let kernel = workspace_root().join(format!(
        "target/{}/{}/kernel-main",
        arch.triple(),
        profile_dir
    ));
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
