use std::{fs, path::Path, path::PathBuf};

use anyhow::{Context, Result, ensure};

const REVISION: &str = "5be26a73d7b7b4d4477d18be94e1d16e615adf56";

pub(super) fn prepare(root: &Path) -> Result<PathBuf> {
    let cache = root.join("cache/limine");
    if assets_exist(&cache) {
        return Ok(cache);
    }

    reset_cache(&cache)?;
    fetch(&cache)?;
    ensure!(assets_exist(&cache), "fixed Limine revision is incomplete");
    Ok(cache)
}

fn assets_exist(cache: &Path) -> bool {
    ["limine-uefi-cd.bin", "BOOTX64.EFI"]
        .iter()
        .all(|file| cache.join(file).is_file())
}

fn reset_cache(cache: &Path) -> Result<()> {
    if cache.exists() {
        println!("==> Removing incomplete Limine cache");
        fs::remove_dir_all(cache).context("failed to remove incomplete Limine cache")?;
    }
    fs::create_dir_all(cache).context("failed to create Limine cache directory")?;
    Ok(())
}

fn fetch(cache: &Path) -> Result<()> {
    println!("==> Fetching Limine");
    crate::cmd!("git -C {cache} init --quiet")?;
    crate::cmd!(
        "git -C {cache} remote add origin https://github.com/limine-bootloader/limine.git"
    )?;
    crate::cmd!("git -C {cache} fetch --depth 1 origin {REVISION}")?;
    crate::cmd!("git -C {cache} checkout --quiet --detach FETCH_HEAD")?;
    Ok(())
}
