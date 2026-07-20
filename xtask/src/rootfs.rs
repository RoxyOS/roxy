use std::{
    fs::{self, File},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, ensure};
use xshell::Shell;

const IMAGE_SIZE: u64 = 32 * 1024 * 1024;

pub(crate) fn get_or_build() -> Result<PathBuf> {
    let workspace = crate::build_kernel::workspace_root();
    let output = output_path(&workspace);

    if output.is_file() {
        println!("==> Reusing root filesystem: {}", output.display());

        return Ok(output);
    }

    build()
}

pub(crate) fn build() -> Result<PathBuf> {
    println!("==> Building root filesystem");

    let workspace = crate::build_kernel::workspace_root();
    let output = output_path(&workspace);
    let staging = workspace.join("target/roxy/rootfs");

    fs::create_dir_all(output.parent().unwrap())
        .context("failed to create root filesystem output directory")?;
    install_base(&workspace, &staging)?;
    create_image(&output, &staging)?;

    ensure!(output.is_file(), "root filesystem image was not produced");

    Ok(output)
}

fn output_path(workspace: &Path) -> PathBuf {
    workspace.join("target/roxy/rootfs.img")
}

fn install_base(workspace: &Path, staging: &Path) -> Result<()> {
    let build_dir = workspace.join("target/jinx");
    let distro = workspace.join("distro");
    let shell = Shell::new()?;

    fs::create_dir_all(&build_dir).context("failed to create Jinx build directory")?;
    shell.change_dir(&build_dir);

    if !build_dir.join(".jinx-parameters").is_file() {
        xshell::cmd!(&shell, "jinx init {distro}")
            .run()
            .context("failed to initialize Jinx build directory")?;
    }

    println!("==> Building base package");
    xshell::cmd!(&shell, "jinx build base")
        .run()
        .context("failed to build base package")?;

    reset_staging(staging)?;

    println!("==> Installing base package");
    xshell::cmd!(&shell, "jinx install {staging} base")
        .run()
        .context("failed to install base package")
}

fn reset_staging(staging: &Path) -> Result<()> {
    if staging.exists() {
        fs::remove_dir_all(staging).context("failed to clear root filesystem staging directory")?;
    }

    fs::create_dir_all(staging).context("failed to create root filesystem staging directory")
}

fn create_image(output: &Path, staging: &Path) -> Result<()> {
    {
        let image = File::create(output).context("failed to create root filesystem image")?;

        image
            .set_len(IMAGE_SIZE)
            .context("failed to size root filesystem image")?;
    }

    crate::cmd!(
        "mke2fs -q -t ext4 -F -b 4096 -I 256 -L roxy-root -O ^has_journal -d {staging} {output}"
    )?;

    Ok(())
}
