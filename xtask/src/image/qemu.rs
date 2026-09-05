use std::{env, path::Path, path::PathBuf, process::Command};

use anyhow::{Result, bail, ensure};

pub(super) fn run(image: &Path) -> Result<()> {
    println!("==> Starting virtual machine");

    let mut command = command(image)?;
    command.arg("-no-shutdown");
    ensure!(command.status()?.success(), "QEMU failed");

    Ok(())
}

pub(super) fn test(image: &Path) -> Result<()> {
    println!("==> Running kernel tests");

    let mut command = command(image)?;
    command.args([
        "-device",
        "isa-debug-exit,iobase=0xf4,iosize=0x04",
        "-display",
        "none",
    ]);

    match command.status()?.code() {
        Some(33) => Ok(()),
        Some(1) => bail!("kernel tests failed"),
        status => bail!("unexpected QEMU termination: {status:?}"),
    }
}

fn command(image: &Path) -> Result<Command> {
    let firmware = firmware()?;
    let mut command = Command::new("qemu-system-x86_64");
    command.args(["-M", "q35"]);

    if cfg!(target_os = "linux") && Path::new("/dev/kvm").exists() {
        command.args(["-enable-kvm", "-cpu", "host"]);
    } else {
        command.args(["-accel", "tcg", "-cpu", "max"]);
    }

    command.arg("-drive").arg(format!(
        "if=pflash,unit=0,format=raw,file={},readonly=on",
        firmware.display()
    ));
    command.arg("-cdrom").arg(image).args([
        "-m",
        "4G",
        "-smp",
        "3",
        "-serial",
        "stdio",
        "-monitor",
        "none",
        "-no-reboot",
    ]);

    Ok(command)
}

fn firmware() -> Result<PathBuf> {
    let firmware = env::var_os("OVMF_CODE").map(PathBuf::from);
    let firmware = firmware.filter(|path| path.is_file());

    ensure!(
        firmware.is_some(),
        "OVMF_CODE must point to an OVMF code image"
    );

    Ok(firmware.unwrap())
}
