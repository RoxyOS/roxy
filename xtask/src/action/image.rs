use anyhow::Result;

use crate::{build_kernel, image_runner};

pub(crate) fn image() -> Result<()> {
    let kernel = build_kernel()?;

    println!("==> Building boot image");
    image_runner(&kernel)?.build_image()?;
    Ok(())
}
