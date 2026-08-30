use spin::Once;

use crate::{FilePermissions, VfsError};

/// Supplies the file mode creation mask of the current process.
///
/// The provider is registered once by process initialization, mirroring how the working
/// directory is injected into path resolution. VFS cannot depend on the process crate, so the
/// kernel registers a `fn` pointer instead. Before registration the default mask is returned,
/// keeping VFS usable independently (e.g. in kernel tests) without process state.
pub type UmaskProvider = fn() -> FilePermissions;

static UMASK_PROVIDER: Once<UmaskProvider> = Once::new();

/// Registers the process umask provider.
///
/// # Errors
///
/// Returns `VfsError::Busy` when a provider has already been registered.
pub fn register_umask_provider(provider: UmaskProvider) -> Result<(), VfsError> {
    if UMASK_PROVIDER.get().is_some() {
        return Err(VfsError::Busy);
    }

    UMASK_PROVIDER.call_once(|| provider);

    Ok(())
}

/// Returns the current process file mode creation mask.
pub(crate) fn current_umask() -> FilePermissions {
    UMASK_PROVIDER
        .get()
        .map_or(FilePermissions::DEFAULT_UMASK, |provider| provider())
}
