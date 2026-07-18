use std::{fs, path::PathBuf};

use anyhow::{Context, Result, bail};

const HEADER: &str = "abi/include/roxy/abi.h";
const CARGO_ARCHIVE: &str = "target/x86_64-unknown-none/release/deps/libroxy_abi.a";
const ARCHIVE: &str = "target/roxy/abi/libroxy-abi.a";

pub(crate) fn build() -> Result<()> {
    check_generated()?;
    build_archive()?;
    check_archive()
}

pub(crate) fn generate() -> Result<()> {
    let header = bindings()?;
    let path = PathBuf::from(HEADER);
    fs::create_dir_all(path.parent().unwrap()).context("create ABI include directory")?;
    fs::write(&path, header).context("write generated ABI header")
}

pub(crate) fn check() -> Result<()> {
    check_generated()?;

    crate::cmd!(
        "clang -std=c11 -Wall -Wextra -Werror -Qunused-arguments -I abi/include -fsyntax-only abi/check/layout.c"
    )?;
    build_archive()?;
    check_archive()
}

fn check_generated() -> Result<()> {
    let generated = bindings()?;
    let committed = fs::read(HEADER).context("read committed ABI header")?;
    if generated != committed {
        bail!("ABI header is stale; run `cargo abi generate`");
    }

    Ok(())
}

fn build_archive() -> Result<()> {
    crate::cmd!(
        "cargo rustc --package roxy-abi --features userspace --target x86_64-unknown-none --release -- --crate-type staticlib -C extra-filename="
    )?;

    let archive = PathBuf::from(ARCHIVE);
    fs::create_dir_all(archive.parent().unwrap()).context("create ABI artifact directory")?;
    fs::copy(CARGO_ARCHIVE, archive).context("copy ABI static library")?;
    Ok(())
}

fn check_archive() -> Result<()> {
    let shell = xshell::Shell::new()?;
    let archive = PathBuf::from(ARCHIVE);
    let symbols = xshell::cmd!(
        &shell,
        "llvm-nm --defined-only --extern-only --just-symbol-name {archive}"
    )
    .read()?;
    if !symbols.lines().any(|symbol| symbol == "roxy_syscall_exit") {
        bail!("ABI static library does not export roxy_syscall_exit");
    }

    Ok(())
}

fn bindings() -> Result<Vec<u8>> {
    let config =
        cbindgen::Config::from_file("abi/cbindgen.toml").map_err(|error| anyhow::anyhow!(error))?;
    let bindings = cbindgen::Builder::new()
        .with_config(config)
        .with_crate("abi")
        .generate()
        .context("generate ABI header")?;
    let mut output = Vec::new();
    bindings.write(&mut output);
    Ok(output)
}
