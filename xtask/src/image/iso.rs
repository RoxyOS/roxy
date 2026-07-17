use std::{fs, path::Path, path::PathBuf};

use anyhow::{Context, Result};

pub(super) fn build(root: &Path, kernel: &Path, limine: &Path) -> Result<PathBuf> {
    let staging = root.join("iso-root");
    let output = root.join("roxy.iso");

    reset_directory(&staging)?;
    stage_kernel(&staging, kernel)?;
    stage_limine(&staging, limine)?;
    create(&staging, &output)?;
    fs::remove_dir_all(staging).context("failed to remove the ISO staging directory")?;
    Ok(output)
}

fn stage_kernel(staging: &Path, kernel: &Path) -> Result<()> {
    copy(kernel, &staging.join("boot/roxy-kernel"))?;
    copy(
        &workspace_root().join("kernel/limine.conf"),
        &staging.join("boot/limine/limine.conf"),
    )
}

fn stage_limine(staging: &Path, limine: &Path) -> Result<()> {
    copy(
        &limine.join("limine-uefi-cd.bin"),
        &staging.join("boot/limine/limine-uefi-cd.bin"),
    )?;
    copy(
        &limine.join("BOOTX64.EFI"),
        &staging.join("EFI/BOOT/BOOTX64.EFI"),
    )
}

fn create(staging: &Path, output: &Path) -> Result<()> {
    println!("==> Creating Limine ISO");
    crate::cmd!(
        "xorriso -as mkisofs -V ROXY --efi-boot boot/limine/limine-uefi-cd.bin -efi-boot-part --efi-boot-image --protective-msdos-label {staging} -o {output}"
    )?;
    Ok(())
}

fn reset_directory(path: &Path) -> Result<()> {
    if path.exists() {
        fs::remove_dir_all(path).context("failed to remove the ISO staging directory")?;
    }
    fs::create_dir_all(path).context("failed to create the ISO staging directory")?;
    Ok(())
}

fn copy(source: &Path, destination: &Path) -> Result<()> {
    fs::create_dir_all(destination.parent().unwrap()).context("failed to create ISO directory")?;
    fs::copy(source, destination).context("failed to stage ISO file")?;
    Ok(())
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned()
}
