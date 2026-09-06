use std::{
    fs::{self, File},
    io::{ErrorKind, Read, Seek, SeekFrom},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, ensure};
use xshell::Shell;

use crate::arch::Arch;

const IMAGE_SIZE: u64 = 512 * 1024 * 1024;
const EXT4_MAGIC_OFFSET: u64 = 1024 + 0x38;
const EXT4_MAGIC: [u8; 2] = [0x53, 0xef];

pub(crate) fn get_or_build(arch: Arch) -> Result<PathBuf> {
    let workspace = crate::build_kernel::workspace_root();
    let output = output_path(&workspace, arch);

    if output.is_file() && has_ext4_superblock(&output)? {
        println!("==> Reusing root filesystem: {}", output.display());

        return Ok(output);
    }

    if output.exists() {
        println!("==> Cached root filesystem is invalid; rebuilding");
    }

    build(arch)
}

pub(crate) fn build(arch: Arch) -> Result<PathBuf> {
    println!("==> Building root filesystem ({})", arch.name());

    let workspace = crate::build_kernel::workspace_root();
    let output = output_path(&workspace, arch);
    let staging = workspace.join("target/roxy/rootfs");

    fs::create_dir_all(output.parent().unwrap())
        .context("failed to create root filesystem output directory")?;
    install_base(&workspace, &staging)?;
    create_standard_dirs(&staging)?;
    create_image(&output, &staging)?;

    ensure!(output.is_file(), "root filesystem image was not produced");

    Ok(output)
}

fn output_path(workspace: &Path, arch: Arch) -> PathBuf {
    workspace.join(format!("target/roxy/rootfs-{}.img", arch.name()))
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

/// Create the standard runtime directory layout. These are image-assembly concerns, not
/// package content, and so are created here rather than in any recipe's `package()` (empty
/// directories are dropped by `xbps-create`).
fn create_standard_dirs(staging: &Path) -> Result<()> {
    for dir in ["run", "proc", "sys", "root", "dev", "var/log"] {
        fs::create_dir_all(staging.join(dir))
            .with_context(|| format!("failed to create staging directory {dir}"))?;
    }
    fs::create_dir_all(staging.join("tmp")).context("failed to create staging directory tmp")?;

    fs::set_permissions(staging.join("tmp"), fs::Permissions::from_mode(0o1777))
        .context("failed to set /tmp permissions")?;
    fs::set_permissions(staging.join("root"), fs::Permissions::from_mode(0o700))
        .context("failed to set /root permissions")?;

    Ok(())
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
