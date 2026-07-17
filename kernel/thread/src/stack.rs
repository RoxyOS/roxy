use alloc::alloc::{alloc_zeroed, dealloc};
use core::{alloc::Layout, ptr::NonNull};

const STACK_SIZE: usize = 64 * 1024;
const STACK_ALIGNMENT: usize = 16;

pub(crate) struct KernelStack {
    pointer: NonNull<u8>,
}

// SAFETY: KernelStack uniquely owns its allocation and exposes no direct memory access.
unsafe impl Send for KernelStack {}

impl KernelStack {
    pub(crate) fn new() -> Option<Self> {
        // SAFETY: STACK_SIZE and STACK_ALIGNMENT form a valid non-zero layout.
        let pointer = unsafe { alloc_zeroed(layout()) };
        NonNull::new(pointer).map(|pointer| Self { pointer })
    }

    pub(crate) fn top_address(&self) -> usize {
        self.pointer.as_ptr() as usize + STACK_SIZE
    }
}

impl Drop for KernelStack {
    fn drop(&mut self) {
        // SAFETY: pointer was allocated with this exact layout and remains uniquely owned.
        unsafe { dealloc(self.pointer.as_ptr(), layout()) };
    }
}

fn layout() -> Layout {
    Layout::from_size_align(STACK_SIZE, STACK_ALIGNMENT).unwrap()
}
