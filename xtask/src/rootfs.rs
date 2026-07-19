use std::{
    fs::{self, File},
    path::PathBuf,
};

use anyhow::{Context, Result, ensure};

const IMAGE_SIZE: u64 = 4 * 1024 * 1024;

pub(crate) fn build() -> Result<PathBuf> {
    println!("==> Building empty root filesystem");

    let output = crate::build_kernel::workspace_root().join("target/roxy/rootfs.img");

    fs::create_dir_all(output.parent().unwrap())
        .context("failed to create root filesystem output directory")?;

    {
        let image = File::create(&output).context("failed to create root filesystem image")?;

        image
            .set_len(IMAGE_SIZE)
            .context("failed to size root filesystem image")?;
    }

    crate::cmd!("mke2fs -q -t ext4 -F -b 4096 -I 256 -L roxy-root -O ^has_journal {output}")?;

    ensure!(output.is_file(), "root filesystem image was not produced");

    Ok(output)
}
