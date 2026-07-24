use std::{fs, path::Path, path::PathBuf};

use anyhow::{Context, Result};

use super::Mode;

pub(super) fn build(
    root: &Path,
    kernel: &Path,
    rootfs: &Path,
    limine: &Path,
    mode: Mode,
) -> Result<PathBuf> {
    let (staging_name, output_name) = match mode {
        Mode::Production => ("iso-root", "roxy.iso"),
        Mode::Test => ("test-iso-root", "roxy-test.iso"),
    };
    let staging = root.join(staging_name);
    let output = root.join(output_name);

    reset_directory(&staging)?;
    stage_kernel(&staging, kernel, rootfs)?;
    stage_limine(&staging, limine)?;
    create(&staging, &output)?;
    fs::remove_dir_all(staging).context("failed to remove the ISO staging directory")?;

    Ok(output)
}

fn stage_kernel(staging: &Path, kernel: &Path, rootfs: &Path) -> Result<()> {
    copy(kernel, &staging.join("boot/kernel-main"))?;
    copy(rootfs, &staging.join("boot/rootfs.img"))?;
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
