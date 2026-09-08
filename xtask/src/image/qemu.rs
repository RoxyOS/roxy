use std::{
    env, fs,
    path::Path,
    path::PathBuf,
    process::{Command, Stdio},
};

use anyhow::{Result, bail, ensure};

use crate::arch::Arch;

/// Launch the normal VM in the foreground, showing a graphical window and the serial
/// console on the invoking terminal.
pub(super) fn run(image: &Path, arch: Arch) -> Result<()> {
    println!("==> Starting virtual machine");

    let mut command = common_command(image, arch)?;
    command.args(["-serial", "stdio", "-monitor", "none", "-no-shutdown"]);
    ensure!(command.status()?.success(), "QEMU failed");

    Ok(())
}

/// Launch the in-kernel test harness headless and exit through QEMU's debug-exit device.
pub(super) fn test(image: &Path, arch: Arch) -> Result<()> {
    println!("==> Running kernel tests");

    let mut command = common_command(image, arch)?;
    command
        .args(["-serial", "stdio", "-monitor", "none"])
        .args([
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

/// Launch the VM detached with every control channel exposed for agent-driven live
/// debugging: a unix socket for the human monitor, a unix socket for QMP, a serial log
/// file, and a GDB stub on a fixed TCP port. See
/// `.pi/skills/live-debugging/SKILL.md` for the matching interaction workflow.
pub(super) fn debug(image: &Path, arch: Arch, dir: &Path) -> Result<()> {
    println!("==> Starting agent-debug virtual machine (detached)");

    // A stale socket path makes QEMU's `server,nowait` fail, so clean any leftovers from
    // a previous session before launching.
    for name in ["monitor.sock", "qmp.sock"] {
        fs::remove_file(dir.join(name)).ok();
    }

    let mut command = common_command(image, arch)?;
    command.args(["-display", "none", "-gdb", "tcp:127.0.0.1:1234"]);
    command
        .arg("-serial")
        .arg(format!("file:{}", dir.join("serial.log").display()))
        .arg("-monitor")
        .arg(format!(
            "unix:{},server,nowait",
            dir.join("monitor.sock").display()
        ))
        .arg("-qmp")
        .arg(format!(
            "unix:{},server,nowait",
            dir.join("qmp.sock").display()
        ));

    // Fully detach: the agent never reads QEMU's own stdout, and QEMU stderr goes to a
    // log file so a failed launch leaves diagnostics behind instead of dying silently.
    command.stdout(Stdio::null());
    let qemu_log = fs::File::create(dir.join("qemu.log"))?;
    command.stderr(Stdio::from(qemu_log));

    let child = command.spawn()?;
    fs::write(dir.join("qemu.pid"), child.id().to_string())?;

    println!(
        "==> channels: monitor   {}",
        dir.join("monitor.sock").display()
    );
    println!("==> channels: qmp       {}", dir.join("qmp.sock").display());
    println!(
        "==> channels: serial    {}",
        dir.join("serial.log").display()
    );
    println!("==> channels: gdb      tcp::1234");
    println!("==> pid: {}", child.id());

    Ok(())
}

/// The shared machine definition for every launch mode: machine model, accelerator, OVMF
/// firmware, boot ISO, and memory/CPU topology. Serial, monitor, and display options are
/// mode-specific and applied by each caller.
fn common_command(image: &Path, arch: Arch) -> Result<Command> {
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
    command
        .arg("-cdrom")
        .arg(image)
        .args(["-m", "4G", "-smp", "16", "-no-reboot"]);

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
