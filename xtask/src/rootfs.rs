use std::{
    fs::{self, File},
    io::{ErrorKind, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, ensure};
use xshell::Shell;

const IMAGE_SIZE: u64 = 128 * 1024 * 1024;
const EXT4_MAGIC_OFFSET: u64 = 1024 + 0x38;
const EXT4_MAGIC: [u8; 2] = [0x53, 0xef];

pub(crate) fn get_or_build() -> Result<PathBuf> {
    let workspace = crate::build_kernel::workspace_root();
    let output = output_path(&workspace);

    if output.is_file() && has_ext4_superblock(&output)? {
        println!("==> Reusing root filesystem: {}", output.display());

        return Ok(output);
    }

    if output.exists() {
        println!("==> Cached root filesystem is invalid; rebuilding");
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

fn has_ext4_superblock(path: &Path) -> Result<bool> {
    let mut image = File::open(path).context("failed to inspect root filesystem image")?;
    let mut magic = [0; EXT4_MAGIC.len()];

    image
        .seek(SeekFrom::Start(EXT4_MAGIC_OFFSET))
        .context("failed to seek root filesystem superblock")?;

    match image.read_exact(&mut magic) {
        Ok(()) => Ok(magic == EXT4_MAGIC),
        Err(error) if error.kind() == ErrorKind::UnexpectedEof => Ok(false),
        Err(error) => Err(error).context("failed to read root filesystem superblock"),
    }
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

    build_base(&shell)?;

    reset_staging(staging)?;

    println!("==> Installing base package");
    xshell::cmd!(&shell, "jinx install {staging} base")
        .run()
        .context("failed to install base package")
}

fn build_base(shell: &Shell) -> Result<()> {
    let pending = xshell::cmd!(shell, "jinx dry-run base")
        .read()
        .context("failed to determine outdated base dependencies")?;
    let dependencies: Vec<_> = pending
        .split_whitespace()
        .filter(|package| *package != "base")
        .collect();

    if !dependencies.is_empty() {
        println!("==> Building outdated base dependencies");
        xshell::cmd!(shell, "jinx build {dependencies...}")
            .run()
            .context("failed to build outdated base dependencies")?;
    }

    println!("==> Building base package");
    xshell::cmd!(shell, "jinx build base")
        .run()
        .context("failed to build base package")
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
