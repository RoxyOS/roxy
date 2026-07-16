use std::{fs, path::Path};

use anyhow::{Context, Result, ensure};
use cargo_image_runner::{Config, ImageRunner, builder};

use crate::build_kernel::workspace_root;

const LIMINE_REVISION: &str = "5be26a73d7b7b4d4477d18be94e1d16e615adf56";
const LIMINE_CACHE_KEY: &str = "v11.x-binary";

pub(crate) fn image_runner(kernel: &Path) -> Result<ImageRunner> {
    println!("==> Configuring image runner");

    let root = workspace_root();
    let mut config = Config::from_toml_file(root.join("kernel/image-runner.toml"))
        .context("failed to load cargo-image-runner configuration")?;
    ensure!(
        config.bootloader.limine.version == LIMINE_CACHE_KEY,
        "Limine revision differs between xtask and image-runner.toml"
    );
    prepare_limine(&root)?;
    let kvm_available = cfg!(target_os = "linux") && Path::new("/dev/kvm").exists();
    config.runner.qemu.kvm = kvm_available;
    if kvm_available {
        config
            .runner
            .qemu
            .extra_args
            .extend(["-cpu".into(), "host".into()]);
    } else {
        config.runner.qemu.extra_args.extend([
            "-accel".into(),
            "tcg".into(),
            "-cpu".into(),
            "max".into(),
        ]);
    }

    builder()
        .with_config(config)
        .workspace_root(root)
        .executable(kernel)
        .build()
        .context("failed to configure cargo-image-runner")
}

// Fetches limine
fn prepare_limine(root: &Path) -> Result<()> {
    let cache = root
        .join("target/image-runner/cache/bootloaders")
        .join(format!("limine-{LIMINE_CACHE_KEY}"));
    if cache.join("BOOTX64.EFI").is_file() && cache.join("limine-uefi-cd.bin").is_file() {
        return Ok(());
    }
    if cache.exists() {
        println!("==> Removing incomplete Limine cache");
        fs::remove_dir_all(&cache).context("failed to remove incomplete Limine cache")?;
    }

    println!("==> Preparing Limine");
    fs::create_dir_all(&cache).context("failed to create Limine cache directory")?;

    println!("==> Fetching Limine");
    crate::cmd!("git -C {cache} init --quiet")?;
    crate::cmd!(
        "git -C {cache} remote add origin https://github.com/limine-bootloader/limine.git"
    )?;
    crate::cmd!("git -C {cache} fetch --depth 1 origin {LIMINE_REVISION}")?;
    crate::cmd!("git -C {cache} checkout --quiet --detach FETCH_HEAD")?;
    ensure!(
        cache.join("BOOTX64.EFI").is_file() && cache.join("limine-uefi-cd.bin").is_file(),
        "fixed Limine revision does not contain the required UEFI assets"
    );
    Ok(())
}
