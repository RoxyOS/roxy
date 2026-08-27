use alloc::sync::Arc;

use roxy_block::{BlockError, RamDisk};
use roxy_boot::BootInfo;
use roxy_devfs::{DevFs, DeviceRegistry};
use roxy_ext4::Ext4FileSystem;
use roxy_vfs::{ResolvedPath, Vfs, VfsError};
use spin::Once;

static ROOT_DEVICE: Once<RamDisk> = Once::new();

pub(crate) fn initialize(boot_info: &BootInfo) -> Result<Arc<DeviceRegistry>, VfsError> {
    let module = boot_info.rootfs_module().ok_or(VfsError::NotFound)?;
    let device =
        ROOT_DEVICE.try_call_once(|| RamDisk::new(module.data).map_err(map_block_error))?;
    let filesystem = Arc::new(Ext4FileSystem::load(device)?);
    let vfs = Vfs::new();

    vfs.mount(ResolvedPath::root(), filesystem)?;

    let device_registry = Arc::new(DeviceRegistry::new());
    let devfs = Arc::new(DevFs::new(device_registry.clone()));

    vfs.mount(ResolvedPath::resolve(b"/dev")?, devfs)?;
    roxy_vfs::register_global_vfs(vfs)?;

    Ok(device_registry)
}

fn map_block_error(error: BlockError) -> VfsError {
    match error {
        BlockError::OutOfBounds | BlockError::Misaligned => VfsError::InvalidInput,
        BlockError::Io => VfsError::Io,
        BlockError::Unsupported => VfsError::Unsupported,
    }
}
