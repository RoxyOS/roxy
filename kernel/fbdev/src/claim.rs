//! Ownership of the visible frame.
//!
//! `/dev/framebuffer` exposes exactly one visible frame, so control is a single claim rather than
//! per-descriptor state: it belongs to the process that took it, any thread of that process may
//! release it, and no other process may take it until then. The claim is keyed on the process, not
//! on an open file description, so it cannot outlive its owner even when the owner never closed
//! the descriptor: the process-exit notification releases it.
//!
//! Taking the frame suspends framebuffer terminal drawing, and releasing it resumes drawing on a
//! cleared screen. Request semantics and error selection follow the DRM master ioctls
//! (`drivers/gpu/drm/drm_auth.c`): taking a frame another process holds fails with `EBUSY`, and
//! releasing one the caller does not hold fails with `EINVAL`.
//!
//! The device is registered once for the single boot framebuffer, so the claim is process-wide
//! state rather than a field of one device object.

use roxy_fd::IoctlError;
use roxy_process::ProcessId;
use roxy_utils::Lock;

static OWNER: Lock<Option<ProcessId>> = Lock::new(None);

/// Takes the visible frame for `caller`.
///
/// Repeat calls from the holder succeed so that a client can assert ownership without tracking
/// whether it already owns the frame.
///
/// # Errors
///
/// Returns [`IoctlError::Busy`] when another process holds the frame.
pub(crate) fn take(caller: ProcessId) -> Result<(), IoctlError> {
    {
        let mut owner = OWNER.lock();

        match *owner {
            Some(holder) if holder == caller => return Ok(()),
            Some(_) => return Err(IoctlError::Busy),
            None => *owner = Some(caller),
        }
    }

    roxy_fbterm::suspend_drawing();

    Ok(())
}

/// Releases the visible frame held by `caller`.
///
/// # Errors
///
/// Returns [`IoctlError::Invalid`] when `caller` does not hold the frame.
pub(crate) fn release(caller: ProcessId) -> Result<(), IoctlError> {
    if clear(caller) {
        Ok(())
    } else {
        Err(IoctlError::Invalid)
    }
}

/// Releases the visible frame when the exiting `process` holds it.
///
/// This is the process-exit notification, which runs while the process table is locked: it must
/// not call back into `roxy-process`, which is why the exiting process ID arrives as an argument.
pub(crate) fn release_exited(process: ProcessId) {
    let _ = clear(process);
}

/// Clears the claim when `process` holds it, reporting whether it did.
fn clear(process: ProcessId) -> bool {
    {
        let mut owner = OWNER.lock();

        if *owner != Some(process) {
            return false;
        }

        *owner = None;
    }

    roxy_fbterm::resume_drawing();

    true
}
