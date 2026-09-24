use std::{
    env, fs,
    net::TcpListener,
    path::Path,
    path::PathBuf,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Result, bail, ensure};

use crate::{arch::Arch, cli::Profile};

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
/// debugging: unix sockets for the human monitor and QMP, a serial log file, and a GDB stub.
/// Each invocation receives its own directory and an automatically selected GDB port. See
/// `.pi/skills/live-debugging/SKILL.md` for the matching interaction workflow.
pub(super) fn debug(
    image: &Path,
    kernel: &Path,
    rootfs: &Path,
    arch: Arch,
    profile: Profile,
    dir: &Path,
) -> Result<()> {
    println!("==> Starting agent-debug virtual machine (detached)");

    let gdb_port = free_tcp_port()?;
    let mut command = common_command(image, arch)?;
    command
        .args(["-display", "none"])
        .arg("-gdb")
        .arg(format!("tcp:127.0.0.1:{gdb_port}"));
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
        ))
        .arg("-d")
        .arg("cpu_reset")
        .arg("-D")
        .arg(dir.join("cpu-reset.log"));

    // Keep a small supervisor as the recorded process so the detached VM's final status can be
    // written after xtask exits. The supervisor also forwards termination to its QEMU child.
    let qemu_program = command.get_program().to_owned();
    let qemu_args: Vec<_> = command.get_args().map(|arg| arg.to_owned()).collect();
    let supervisor_script = write_supervisor_script(dir)?;
    let exit_status = dir.join("exit-status");
    let qemu_log = fs::File::create(dir.join("qemu.log"))?;
    let mut supervisor = Command::new("sh");
    supervisor
        .arg(&supervisor_script)
        .arg(qemu_program)
        .args(qemu_args)
        .env("ROXY_EXIT_STATUS", &exit_status)
        .stdout(Stdio::null())
        .stderr(Stdio::from(qemu_log));

    let mut child = supervisor.spawn()?;
    let pid = child.id();
    fs::write(dir.join("qemu.pid"), pid.to_string())?;
    write_manifest(
        dir,
        pid,
        gdb_port,
        profile,
        image,
        kernel,
        rootfs,
        &exit_status,
        &supervisor_script,
    )?;

    if let Some(status) = child.try_wait()? {
        bail!("QEMU supervisor exited during launch with status {status}");
    }
    if let Err(error) = wait_for_qmp_socket(&mut child, &dir.join("qmp.sock")) {
        let _ = child.kill();
        let _ = child.wait();
        return Err(error);
    }

    println!("==> session: {}", dir.display());
    println!(
        "==> channels: monitor   {}",
        dir.join("monitor.sock").display()
    );
    println!("==> channels: qmp       {}", dir.join("qmp.sock").display());
    println!(
        "==> channels: serial    {}",
        dir.join("serial.log").display()
    );
    println!("==> channels: gdb       tcp:127.0.0.1:{gdb_port}");
    println!("==> pid: {pid}");

    Ok(())
}

fn wait_for_qmp_socket(child: &mut Child, socket: &Path) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(2);
    while !socket.exists() {
        if let Some(status) = child.try_wait()? {
            bail!("QEMU supervisor exited before QMP startup with status {status}");
        }
        if Instant::now() >= deadline {
            bail!("QMP socket did not appear: {}", socket.display());
        }
        thread::sleep(Duration::from_millis(25));
    }
    Ok(())
}

fn write_supervisor_script(dir: &Path) -> Result<PathBuf> {
    let path = dir.join("supervisor.sh");
    fs::write(
        &path,
        "#!/bin/sh\nset -u\n\"$@\" &\nchild=$!\ntrap 'kill \"$child\" 2>/dev/null || true' TERM INT HUP\nset +e\nwait \"$child\"\nstatus=$?\nset -e\nprintf 'exit_code=%s\\n' \"$status\" > \"$ROXY_EXIT_STATUS\"\nexit \"$status\"\n",
    )?;
    Ok(path)
}

fn free_tcp_port() -> Result<u16> {
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    Ok(listener.local_addr()?.port())
}

fn write_manifest(
    dir: &Path,
    pid: u32,
    gdb_port: u16,
    profile: Profile,
    image: &Path,
    kernel: &Path,
    rootfs: &Path,
    exit_status: &Path,
    supervisor: &Path,
) -> Result<()> {
    let manifest = format!(
        r#"{{
  "pid": {pid},
  "profile": "{}",
  "gdb": "tcp:127.0.0.1:{gdb_port}",
  "iso": "{}",
  "kernel": "{}",
  "rootfs": "{}",
  "qmp": "{}",
  "monitor": "{}",
  "serial": "{}",
  "cpu_reset_log": "{}",
  "exit_status": "{}",
  "supervisor": "{}"
}}
"#,
        profile.name(),
        json_path(image),
        json_path(kernel),
        json_path(rootfs),
        json_path(&dir.join("qmp.sock")),
        json_path(&dir.join("monitor.sock")),
        json_path(&dir.join("serial.log")),
        json_path(&dir.join("cpu-reset.log")),
        json_path(exit_status),
        json_path(supervisor),
    );
    fs::write(dir.join("manifest.json"), manifest)?;
    Ok(())
}

fn json_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
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
