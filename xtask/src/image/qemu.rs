use std::{env, path::Path, path::PathBuf};

use anyhow::{Result, ensure};

pub(super) fn run(image: &Path) -> Result<()> {
    println!("==> Starting virtual machine");

    let firmware = firmware()?;
    if cfg!(target_os = "linux") && Path::new("/dev/kvm").exists() {
        crate::cmd!(
            "qemu-system-x86_64 -M q35 -enable-kvm -cpu host -drive if=pflash,unit=0,format=raw,file={firmware},readonly=on -cdrom {image} -m 256M -smp 1 -serial stdio -monitor none -no-reboot -no-shutdown -display none"
        )?;
    } else {
        crate::cmd!(
            "qemu-system-x86_64 -M q35 -accel tcg -cpu max -drive if=pflash,unit=0,format=raw,file={firmware},readonly=on -cdrom {image} -m 256M -smp 1 -serial stdio -monitor none -no-reboot -no-shutdown -display none"
        )?;
    }
    Ok(())
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
