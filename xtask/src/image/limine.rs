use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, ensure};

const VERSION: &str = "v12.7.0";

pub(super) fn prepare(root: &Path) -> Result<PathBuf> {
    let cache = root.join("cache/limine");

    if cache_valid(&cache) {
        return Ok(cache);
    }

    reset_cache(&cache)?;
    fetch(&cache)?;
    mark_version(&cache)?;
    ensure!(cache_valid(&cache), "Limine {VERSION} fetch is incomplete");

    Ok(cache)
}

fn cache_valid(cache: &Path) -> bool {
    let version = fs::read_to_string(cache.join("version")).unwrap_or_default();
    version.trim() == VERSION
        && ["limine-uefi-cd.bin", "BOOTX64.EFI"]
            .iter()
            .all(|file| cache.join(file).is_file())
}

fn mark_version(cache: &Path) -> Result<()> {
    let mut file = File::create(cache.join("version"))?;
    file.write_all(VERSION.as_bytes())?;
    Ok(())
}

fn reset_cache(cache: &Path) -> Result<()> {
    if cache.exists() {
        println!("==> Removing stale Limine cache");
        fs::remove_dir_all(cache).context("failed to remove stale Limine cache")?;
    }
    fs::create_dir_all(cache).context("failed to create Limine cache directory")?;

    Ok(())
}

fn fetch(cache: &Path) -> Result<()> {
    println!("==> Fetching Limine {VERSION}");

    let url = format!(
        "https://github.com/limine-bootloader/limine/releases/download/{VERSION}/\
         limine-binary.tar.xz"
    );
    let tarball = cache.join("limine-binary.tar.xz");

    crate::cmd!("wget -q -O {tarball} {url}")?;
    crate::cmd!("tar -xJf {tarball} -C {cache} --strip-components=1")?;
    fs::remove_file(&tarball).context("failed to remove Limine tarball")?;

    Ok(())
}
