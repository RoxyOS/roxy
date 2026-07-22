use alloc::sync::Arc;

use crate::PhysicalAddress;

use super::allocator::{self, FrameIndex};

const PAGE_BYTES: usize = 4096;

pub struct OwnedFrame {
    frame: FrameIndex,
    owned: bool,
}

impl OwnedFrame {
    pub(super) const fn new(frame: FrameIndex) -> Self {
        Self { frame, owned: true }
    }

    #[must_use]
    pub fn start_address(&self) -> PhysicalAddress {
        self.frame.start_address()
    }

    #[must_use]
    pub fn into_page_ref(mut self) -> PageRef {
        self.owned = false;
        PageRef(Arc::new(FrameOwner { frame: self.frame }))
    }

    pub(crate) fn zero(&self) {
        write_frame(self.frame, 0, &ZERO_PAGE).unwrap();
    }

    pub(crate) fn transfer_to_mapping(mut self) {
        self.owned = false;
    }
}

impl Drop for OwnedFrame {
    fn drop(&mut self) {
        if self.owned {
            allocator::deallocate(self.frame);
        }
    }
}

#[derive(Clone)]
pub struct PageRef(Arc<FrameOwner>);

impl PageRef {
    #[must_use]
    pub fn start_address(&self) -> PhysicalAddress {
        self.0.frame.start_address()
    }

    /// Duplicates the complete physical page into a newly allocated frame.
    ///
    /// Returns `None` when the frame allocator cannot satisfy the request.
    #[must_use]
    pub fn duplicate(&self) -> Option<Self> {
        let destination = OwnedFrame::new(allocator::allocate()?);
        let source = allocator::physical_pointer::<u8>(self.start_address());
        let destination_pointer = allocator::physical_pointer::<u8>(destination.start_address());

        // SAFETY: both pointers identify live, page-sized physical frames owned by this method
        // and the source page reference; the ranges do not overlap.
        unsafe { source.copy_to_nonoverlapping(destination_pointer, PAGE_BYTES) };

        Some(destination.into_page_ref())
    }

    /// Reads bytes from this frame.
    ///
    /// # Safety
    ///
    /// The caller must prevent concurrent access through any active virtual mapping.
    ///
    /// # Errors
    ///
    /// Returns an error when the requested range exceeds the frame.
    pub unsafe fn read(&self, offset: usize, output: &mut [u8]) -> Result<(), FrameAccessError> {
        validate_range(offset, output.len())?;
        let source = allocator::physical_pointer::<u8>(self.start_address()).wrapping_add(offset);

        // SAFETY: validate_range keeps the read inside this live frame, which PageRef owns.
        unsafe { source.copy_to_nonoverlapping(output.as_mut_ptr(), output.len()) };

        Ok(())
    }

    /// Writes bytes to this uniquely owned frame.
    ///
    /// # Safety
    ///
    /// The caller must prevent concurrent access through any active virtual mapping.
    ///
    /// # Errors
    ///
    /// Returns an error when the range exceeds the frame or this reference was cloned.
    pub unsafe fn write(&mut self, offset: usize, input: &[u8]) -> Result<(), FrameAccessError> {
        let owner = Arc::get_mut(&mut self.0).ok_or(FrameAccessError::Shared)?;
        write_frame(owner.frame, offset, input)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameAccessError {
    OutOfBounds,
    Shared,
}

struct FrameOwner {
    frame: FrameIndex,
}

impl Drop for FrameOwner {
    fn drop(&mut self) {
        allocator::deallocate(self.frame);
    }
}

const ZERO_PAGE: [u8; PAGE_BYTES] = [0; PAGE_BYTES];

fn write_frame(frame: FrameIndex, offset: usize, input: &[u8]) -> Result<(), FrameAccessError> {
    validate_range(offset, input.len())?;
    let destination = allocator::physical_pointer::<u8>(frame.start_address()).wrapping_add(offset);

    // SAFETY: validate_range keeps the write inside this uniquely allocated live frame.
    unsafe {
        input
            .as_ptr()
            .copy_to_nonoverlapping(destination, input.len());
    };

    Ok(())
}

fn validate_range(offset: usize, length: usize) -> Result<(), FrameAccessError> {
    let end = offset
        .checked_add(length)
        .ok_or(FrameAccessError::OutOfBounds)?;
    (end <= PAGE_BYTES)
        .then_some(())
        .ok_or(FrameAccessError::OutOfBounds)
}
