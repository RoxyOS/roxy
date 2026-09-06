use std::{env, path::Path, path::PathBuf, process::Command};

use anyhow::{Result, bail, ensure};

use crate::arch::Arch;

pub(super) fn run(image: &Path, arch: Arch) -> Result<()> {
    println!("==> Starting virtual machine");

    let mut command = command(image, arch)?;
    command.arg("-no-shutdown");
    ensure!(command.status()?.success(), "QEMU failed");

    Ok(())
}

pub(super) fn test(image: &Path, arch: Arch) -> Result<()> {
    println!("==> Running kernel tests");

    let mut command = command(image, arch)?;
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

fn command(image: &Path, arch: Arch) -> Result<Command> {
    let firmware = firmware(arch)?;
    let mut command = Command::new(arch.qemu_runner());
    command.args(["-M", arch.qemu_machine()]);

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
        "16",
        "-serial",
        "stdio",
        "-monitor",
        "none",
        "-no-reboot",
    ]);

    Ok(command)
}

fn firmware(arch: Arch) -> Result<PathBuf> {
    match arch {
        Arch::X86_64 => {
            let firmware = env::var_os("OVMF_CODE").map(PathBuf::from);
            let firmware = firmware.filter(|path| path.is_file());

            ensure!(
                firmware.is_some(),
                "OVMF_CODE must point to an OVMF code image"
            );

            Ok(firmware.unwrap())
        }
        Arch::Aarch64 => {
            bail!("aarch64 boot is not yet wired: the runner has no firmware/EFI path for aarch64")
        }
    }
}
