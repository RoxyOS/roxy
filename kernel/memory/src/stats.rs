#[derive(Clone, Copy, Debug)]
pub struct MemoryStats {
    pub total_frames: usize,
    pub allocated_frames: usize,
    pub heap_total_bytes: usize,
    pub heap_requested_bytes: usize,
    pub heap_allocated_bytes: usize,
    pub frame_allocation_attempts: usize,
    pub heap_allocation_attempts: usize,
}

impl MemoryStats {
    #[must_use]
    pub const fn free_frames(self) -> usize {
        self.total_frames - self.allocated_frames
    }
}
